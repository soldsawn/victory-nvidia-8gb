# GTX 1660 6 GB / Windows 11 21H2 results (v9, 1080p)

Date: 2026-11-26 (MSK)

Build: `soldsawn/victory-nvidia-8gb@7b7b952` (fxceless `ca8829f`, including `3973179`, `465b94f`, `ca8829f`, plus #854 and the 8 GB VRAM fixes), Series X profile, `VICTORY_RESOLUTION=1920x1080`, `VICTORY_TEXTURE_DROP_MIPS=2`. Agility SDK 620 (`D3D12Core.dll` beside the exe; `apps/victory/src/agility.rs` + `build.rs` export `D3D12SDKVersion`/`D3D12SDKPath`).

## Hardware

- Windows 11 21H2 (10.0.22000), Intel Core i7-4790K (4 cores / 8 threads, 4.0 GHz), 24 GB RAM, NVIDIA GeForce GTX 1660 6 GB (Turing, 32-lane/Wave32), driver 617.42, 1920x1080 display.
- Series X profile, 4K display mode, `VICTORY_RESOLUTION=1920x1080`, `VICTORY_TEXTURE_DROP_MIPS=2`, and the v9 launcher's switches (`ASYNC_SUBMIT`, `QUEUE2`, `INDIRECT_WINDOW_DRAWS`, `INDIRECT_GATHER`, `JOIN_PARTS`, `BACK_OWN_VIEW`, `GUARD_HOLES_FRAME`, `HEAP_WORKER`, `ALIAS_ONCE`, `TEXTURE_NO_WAIT`, `COPY_PLAIN`, `SKIP_WC_COMPARE`, `WIDE_TEXTURES`, `DISK_CORES=slow`, `XMEM_PERSISTENT_VIEW`).

## Results (same machine and save)

| Build | fps / frame | Draws | GPU | VRAM (card's own) | Notes |
| --- | --- | --- | --- | --- | --- |
| v9, 1080p + DropMips=2 | **5-6 fps / 181-240 ms** | 4,187 | 51 % (14-69 ms) | **3.98 / 4.93 GB** | CPU 90 % (the limit); 4,009-8,393 textures waiting; 0 wave64/root-signature/pipeline refusals; picture normal, black skin fixed |
| v8, 4K (same tree without the 1080p cap) | 1.7-3.3 fps / 302-577 ms | 3,863-4,657 | 14-100 % (varies) | 6.35 / 4.93 GB (over) | 7,475-15,385 textures waiting; 0 refusals; picture normal |
| pure `465b94f` (no #854, no 1080p) | 2.0-2.5 fps / 404-501 ms | 2,848-4,136 | 237-259 ms | 6.6 / 4.93 GB (over) | 7 wave64 refusals; black face |

The 1080p cap plus `TEXTURE_DROP_MIPS=2` is the decisive change on this card: it keeps VRAM inside the 4.9 GB budget and moves the limit from the GPU to the CPU. The RTX 4060 Laptop result (8.7 fps / ~115 ms) was measured on an i7-14700KF; this machine's i7-4790K is at 90 % CPU in the world.

## Slot 0x218 (state objects) - not included

Applying `sendronbrian02-sudo@dea50eda` (slot 0x218 creates a pipeline from the stream the title passes) made 2,048 stream pipelines with S_OK, then the title crashed at the world/load with `unhandled 0xC0000005 at victory-image+0x164a298 (read 0x2c: nothing mapped)`. The no-stage state objects are dereferenced by the title. Reverted; the v9 tree does not contain it.

## Remaining

- No facial animations; the boat sinks underwater; cutscene boats/people issues. These are not fixed by v9.
- The CPU (i7-4790K) is the limit at 5-6 fps; the GPU sits at 51 %.

## Windows 11 21H2 note

On Windows 11 21H2 the in-box D3D12 refuses the root-signature flags `0xC01` (`ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT | CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED | SAMPLER_HEAP_DIRECTLY_INDEXED`): every graphics root signature is refused and the title crashes. Exporting `D3D12SDKVersion` 620 and `D3D12SDKPath` with `D3D12Core.dll` 620 beside the exe fixes it. The `apps/victory/src/agility.rs`, `apps/victory/build.rs` and the `mod agility;` in `main.rs` in this branch are that fix; `D3D12Core.dll` itself is not committed.

No game files, shaders, textures, audio or logs are included. Tested and written with AI assistance.
