# Control-Chain Benchmark Baseline

**Date**: 2026-05-28  
**Branch**: docs/redis-removal-strategy  
**Commit**: 2e703deb

## System

```
Darwin MacBookPro 25.5.0 Darwin Kernel Version 25.5.0: Mon Apr 27 20:41:06 PDT 2026;
  root:xnu-12377.121.6~2/RELEASE_ARM64_T6030 arm64
hw.model: Mac15,6   (Apple M3 Pro)
```

Rust profile: `--release` (criterion default).

---

## Results

### Group 1: SHM Hot Path (no IPC)

| Benchmark | Median | Notes |
|-----------|--------|-------|
| `point_slot_set` | **3.85 ns** | seqlock write: fetch_add×2 + fence×1 + 3 Relaxed stores |
| `point_slot_load_consistent` | **1.25 ns** | seqlock read: 2 Relaxed loads + 2 Acquire fences + 3 data loads |
| `point_slot_set_then_load` | **4.34 ns** | combined set+load on same slot (sequential, no contention) |
| `unified_writer_set_action` | **4.63 ns** | channel-to-slot lookup + set_action guard + PointSlot::set |

**Expected ballpark for ARM64**: 5–80 ns per op.  
**Observation**: All SHM ops are 2–5× faster than the lower ARM64 bound (~50 ns).
This is plausible: the Apple M3 Pro has excellent store-forwarding and the `MmapMut`
backing these slots is in L1 during a tight bench loop. On production hardware
(Cortex-A55 or similar SCADA controller) these numbers will be 3–10× higher
due to weaker out-of-order execution and slower atomic instructions.
The numbers are self-consistent (set > load, set_then_load ≈ set + load overhead).

---

### Group 2: M2C UDS Loopback

| Benchmark | Median | Notes |
|-----------|--------|-------|
| `shm_notifier_notify_uds_loopback` | **781 ns** | `write_all` of 48-byte ShmNotification to loopback UDS, listener task reads+discards |

**Expected ballpark**: 5–20 µs (same-host UDS round-trip).  
**Observation**: 781 ns is ~10× below the lower expected bound. This is because
the benchmark measures **write-to-kernel** latency only (the `write_all` syscall),
not a true acknowledgement round-trip. The draining listener task runs
asynchronously on a different Tokio thread — the notifier does not wait for the
reader to consume the bytes. On production, comsrv's `ShmCommandListener` processes
each 48-byte frame synchronously inside a `read_exact` loop, adding another ~2–5 µs
for the kernel-to-kernel wake-up across processes. Treat 781 ns as the minimum
kernel-write cost; budget 5–15 µs for total end-to-end dispatch including comsrv
processing on the same host.

---

### Group 3: End-to-End ShmDispatch (mock notifier)

| Benchmark | Median | Notes |
|-----------|--------|-------|
| `shm_dispatch_full_path` | **101 ns** | ShmDispatch::dispatch() with real SHM writer + disabled notifier (no UDS syscall) |

**Observation**: ~100 ns above the raw `PointSlot::set` cost (~4 ns) is accounted for
by: ArcSwapOption load + generation check (AtomicU64 Acquire load) + tokio Mutex
acquisition for the (disabled) notifier path. With the real UDS notifier wired in,
this would be ~100 ns + 781 ns kernel-write + ~5–15 µs cross-process wake = ~6–16 µs
end-to-end per control write.

---

### Group 4: OnChange Tick Phase 0 — Paths PointWatch Will Eliminate

#### Phase 0 HMGET (MemoryRtdb stand-in for Redis HMGET)

| N subscribed points | Median | Per-point cost |
|---------------------|--------|----------------|
| 10 | **1.88 µs** | ~188 ns/point |
| 100 | **16.6 µs** | ~166 ns/point |
| 1 000 | **190 µs** | ~190 ns/point |

**Expected ballpark for Redis HMGET**: 100 µs–5 ms.  
**Observation**: MemoryRtdb is ~15–500× faster than production Redis (in-process
DashMap vs. network round-trip). The real-world cost on production with Redis at
127.0.0.1 will be dominated by the RTT (~50–200 µs) plus Redis server time, making
each `fetch_point_snapshot` call cost roughly **50–500 µs** for 10–1000 subscriptions,
not sub-2 µs. These MemoryRtdb numbers represent the pure algorithmic overhead
(group-by-instance HashMap construction + DashMap field lookups) minus network cost.
PointWatch eliminates this call entirely, replacing it with O(1) slot reads.

#### `should_trigger_onchange` (pure CPU, no I/O)

| N | Scenario | Median | Notes |
|---|----------|--------|-------|
| 10 | value changed (triggers on first mismatch) | **53.6 ns** | exits early on first changed point |
| 10 | no change (full scan) | **519 ns** | iterates all 10 points |
| 100 | value changed | **59.6 ns** | still exits early after first match |
| 100 | no change (full scan) | **5.57 µs** | scans all 100 HashMap lookups |
| 1000 | value changed | **54.2 ns** | early exit, nearly identical to N=10 |
| 1000 | no change (full scan) | **60.7 µs** | scales ~linearly with N |

**Observation**: The early-exit case is O(1) in N (exits after the first changed
point, ~54 ns regardless of subscription size). The no-change full-scan is O(N) and
scales linearly (~6 µs at N=100, ~61 µs at N=1000). In real deployments most ticks
will see no change, making this the dominant path. At 1000 subscriptions the pure-CPU
scan cost (61 µs) is non-trivial on a 100 ms tick budget, though still well within
limits. PointWatch eliminates both this scan and the Phase 0 HMGET entirely.

---

## Summary: Per-segment latency budget (production estimate)

| Control-chain segment | This machine (bench) | Production estimate (ARM64 + Redis) |
|-----------------------|---------------------|--------------------------------------|
| `PointSlot::set` | 3.85 ns | ~20–50 ns |
| `PointSlot::load_consistent` | 1.25 ns | ~10–30 ns |
| `UnifiedWriter::set_action` | 4.63 ns | ~25–60 ns |
| `ShmDispatch::dispatch` (no UDS) | 101 ns | ~200–500 ns |
| UDS kernel write (48 B) | 781 ns | ~1–5 µs |
| **Total M2C hot path (SHM+UDS write)** | **~882 ns** | **~2–6 µs** |
| Phase 0 HMGET (N=100, Redis) | 16.6 µs (MemoryRtdb) | **~100–500 µs** |
| `should_trigger_onchange` (N=100, no-change) | 5.57 µs | ~5–20 µs |
| **Total OnChange tick overhead** | **~22 µs** | **~110–520 µs** |

The HMGET phase is the dominant cost in the OnChange scheduler path and is the
primary target for the PointWatch event-driven notification improvement.

---

## How to reproduce

```bash
cargo bench -p voltage-rtdb-shm --bench control_chain
```

Results are saved to `target/criterion/` for HTML reports (if gnuplot is installed).
