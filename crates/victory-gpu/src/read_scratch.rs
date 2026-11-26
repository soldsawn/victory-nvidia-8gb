use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};

/// Retain the ordinary reads, while freeing outlier buffers after use. This
/// follows the upstream Victory texture scratch policy at 13f91ca and checks
/// capacity as well as length, since Vec may grow geometrically.
const KEEP_BYTES: usize = 32 << 20;
const SHARED_MAX: usize = 64 << 20;
static SHARED: SharedScratch = SharedScratch(Mutex::new(None));

#[cfg(test)]
pub(crate) fn shared_retained() -> usize {
    SHARED.retained()
}

/// One idle large buffer process-wide; in-flight reads never hold the lock.
#[derive(Default)]
pub(crate) struct SharedScratch(Mutex<Option<Vec<u8>>>);

impl SharedScratch {
    fn clear(&self) {
        // Drop outside the mutex: returning virtual memory can be slow.
        let buffer = self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(buffer);
    }

    fn with_large<R>(&self, size: usize, f: impl FnOnce(&mut [u8]) -> R) -> R {
        let mut buffer = self.0.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
        if buffer.capacity() < size {
            buffer.reserve_exact(size - buffer.len());
        }
        buffer.resize(size, 0);
        let result = f(&mut buffer);
        if buffer.capacity() <= SHARED_MAX {
            let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if slot.is_none() {
                *slot = Some(buffer);
            }
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn with_buffer<R>(&self, cache: &RefCell<Vec<u8>>, size: usize, f: impl FnOnce(&mut [u8]) -> R) -> R {
        if size > SHARED_MAX {
            self.clear();
            return with_bounded_buffer(cache, size, f);
        }
        if size <= KEEP_BYTES {
            return with_bounded_buffer(cache, size, f);
        }
        self.with_large(size, f)
    }

    #[cfg(test)]
    pub(crate) fn retained(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(0, Vec::capacity)
    }
}

pub(crate) fn with_buffer<R>(cache: &RefCell<Vec<u8>>, size: usize, f: impl FnOnce(&mut [u8]) -> R) -> R {
    if size > KEEP_BYTES {
        static ON: OnceLock<bool> = OnceLock::new();
        if *ON.get_or_init(|| std::env::var_os("VICTORY_SCRATCH_SHARED").is_some_and(|v| v == "1")) {
            if size <= SHARED_MAX {
                return SHARED.with_large(size, f);
            }
            // Do not carry idle large memory into an even larger request.
            // Concurrent in-flight reads may return another buffer later.
            SHARED.clear();
        }
    }
    with_bounded_buffer(cache, size, f)
}

fn with_bounded_buffer<R>(cache: &RefCell<Vec<u8>>, size: usize, f: impl FnOnce(&mut [u8]) -> R) -> R {
    if size > KEEP_BYTES {
        return f(&mut vec![0; size]);
    }
    let mut buffer = cache.borrow_mut();
    if buffer.len() < size {
        if buffer.capacity() < size && buffer.capacity() > KEEP_BYTES / 2 {
            let extra = size - buffer.len();
            buffer.reserve_exact(extra);
        }
        buffer.resize(size, 0);
    }
    f(&mut buffer[..size])
}

#[cfg(test)]
pub(crate) mod benchmarks {
    use super::{with_bounded_buffer, SharedScratch};
    use std::cell::RefCell;
    use std::time::Instant;

    pub(crate) type SharedPrototype = SharedScratch;

    #[test]
    fn shared_mode_is_opt_in_and_reuses_the_production_pool() {
        if std::env::var_os("VICTORY_SCRATCH_TEST_CHILD").is_some() {
            let enabled = std::env::var_os("VICTORY_SCRATCH_SHARED").is_some();
            let cache = RefCell::new(Vec::new());
            super::with_buffer(&cache, 33 << 20, |bytes| bytes.fill(0x5a));
            let retained = super::SHARED.retained();
            assert_eq!(retained != 0, enabled);
            if enabled {
                assert!(retained <= 64 << 20);
                super::with_buffer(&cache, 33 << 20, |bytes| assert_eq!(bytes[0], 0x5a));
                assert_eq!(super::SHARED.retained(), retained);
                super::with_buffer(&cache, 65 << 20, |_| assert_eq!(super::SHARED.retained(), 0));
                assert_eq!(super::SHARED.retained(), 0);
            }
            return;
        }
        let exe = std::env::current_exe().unwrap();
        for enabled in [false, true] {
            let mut child = std::process::Command::new(&exe);
            child.args(["--exact", "read_scratch::benchmarks::shared_mode_is_opt_in_and_reuses_the_production_pool", "--nocapture"])
                .env("VICTORY_SCRATCH_TEST_CHILD", "1")
                .env_remove("VICTORY_SCRATCH_SHARED");
            if enabled {
                child.env("VICTORY_SCRATCH_SHARED", "1");
            }
            let output = child.output().unwrap();
            assert!(output.status.success(), "child {enabled}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    fn shared_pool_does_not_keep_oversized_buffers() {
        let shared = SharedScratch::default();
        let cache = RefCell::new(Vec::new());
        shared.with_buffer(&cache, 65 << 20, |bytes| bytes[0] = 1);
        assert_eq!(shared.retained(), 0);
        assert_eq!(cache.borrow().capacity(), 0);
    }

    #[test]
    #[ignore = "synthetic concurrent allocation test; uses 132 MiB"]
    fn shared_prototype_handles_concurrent_reads() {
        let shared = SharedPrototype::default();
        let barrier = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for byte in 1..=4u8 {
                let shared = &shared;
                let barrier = &barrier;
                scope.spawn(move || {
                    let cache = RefCell::new(Vec::new());
                    shared.with_buffer(&cache, 33 << 20, |buffer| {
                        buffer.fill(byte);
                        barrier.wait();
                        assert!(buffer.iter().all(|&b| b == byte));
                    });
                });
            }
        });
        let retained = shared.0.lock().unwrap().as_ref().map_or(0, Vec::capacity);
        assert!((33 << 20..=64 << 20).contains(&retained));
    }

    /// Repeated full writes model a read that fills its destination. This
    /// measures allocator and memory-touch costs, not actual texture upload.
    #[test]
    #[ignore = "synthetic throughput measurement; run alone in release mode"]
    fn repeated_filled_reads() {
        const MIB: usize = 1 << 20;
        const ITERATIONS: usize = 16;
        for size_mib in [16, 32, 33, 64] {
            for policy in ["retained", "bounded", "shared_prototype"] {
                let cache = RefCell::new(Vec::<u8>::new());
                let shared = SharedPrototype::default();
                let size = size_mib * MIB;
                let start = Instant::now();
                for turn in 0..ITERATIONS {
                    let fill = |buffer: &mut [u8]| {
                        buffer.fill(turn as u8);
                        std::hint::black_box(buffer);
                    };
                    if policy == "shared_prototype" {
                        shared.with_buffer(&cache, size, fill);
                    } else if policy == "bounded" {
                        with_bounded_buffer(&cache, size, fill);
                    } else {
                        let mut buffer = cache.borrow_mut();
                        buffer.resize(size, 0);
                        fill(&mut buffer);
                    }
                }
                let millis = start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
                let pooled = shared.retained();
                println!("size_mib={size_mib} policy={policy} mean_ms={millis:.3} retained_mib={:.1}", (cache.borrow().capacity() + pooled) as f64 / MIB as f64);
            }
        }
    }
}
