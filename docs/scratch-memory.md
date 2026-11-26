# Scratch memory

Texture reads and some shadow-buffer reads use per-thread scratch memory. By default, each thread retains up to 32 MiB and releases larger temporary buffers after use. Large concurrent reads can still require much more memory while they are running.

For an experimental CPU-versus-memory tradeoff, set `VICTORY_SCRATCH_SHARED=1` before starting VIctory. Reads larger than 32 MiB and up to 64 MiB can reuse one shared buffer. At most one such idle buffer is kept process-wide, in addition to the per-thread caches. Simultaneous readers may allocate additional temporary buffers. A request over 64 MiB releases the idle shared buffer before allocating; a concurrent reader can return another buffer afterward. Buffers over 64 MiB are not pooled. The setting is off by default.

Repeated synthetic host-copy tests showed lower CPU time with reuse, at the cost of retaining up to 64 MiB more memory. Actual title request sizes, memory pressure, and performance have not been measured. The option does not set a process RAM limit.
