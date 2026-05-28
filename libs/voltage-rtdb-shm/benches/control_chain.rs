//! Control-chain microbenchmarks for VoltageEMS.
//!
//! Measures the latency of each segment in the M2C control chain:
//!   modsrv detects anomaly via SHM → rule engine evaluates
//!     → modsrv writes C/A point to SHM → UDS notify comsrv
//!       → comsrv dispatches to protocol adapter
//!
//! Run with:
//!   cargo bench -p voltage-rtdb-shm --bench control_chain
//!
//! # Groups
//! 1. `shm_hot_path`         — PointSlot set/load, UnifiedWriter::set_action
//! 2. `m2c_uds`              — ShmNotifier round-trip to loopback UDS listener
//! 3. `shm_dispatch`         — Full ShmDispatch::dispatch() with mock notifier
//! 4. `onchange_tick_phase0` — fetch_point_snapshot / should_trigger_onchange
//!    (the paths PointWatch eliminates)

#![allow(clippy::disallowed_methods)] // benchmark code – unwrap() is fine

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use voltage_routing::RouteContext;
use voltage_rtdb::MemoryRtdb;
use voltage_rtdb::Rtdb;
use voltage_rtdb_shm::{
    ActionDispatch, ChannelPointCounts, DispatchOutcome, PointSlot, SharedConfig, ShmDispatch,
    ShmNotifier, UnifiedWriter,
};

// ─────────────────────────────────────────────────────────────────────────────
// Shared setup helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal channel layout: channel 1001 with 4 T + 1 C + 1 A points.
fn bench_channel_points() -> ChannelPointCounts {
    let mut map = BTreeMap::new();
    // [T, S, C, A]
    map.insert(1001u32, [4u32, 0, 1, 1]);
    ChannelPointCounts::from_map(map)
}

fn bench_config(dir: &std::path::Path) -> SharedConfig {
    SharedConfig::default()
        .with_path(dir.join("bench.shm"))
        .with_max_slots(1024)
}

/// A `RouteContext` targeting channel 1001, Control point 0.
fn bench_route_ctx() -> RouteContext {
    RouteContext {
        channel_id: "1001".to_string(),
        point_type: "C".to_string(),
        comsrv_point_id: "0".to_string(),
        target_channel_id: 1001,
        target_point_type: 2, // Control
        target_point_id: 0,
        timestamp_ms: 1_748_000_000_000i64,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Group 1: SHM hot path (no IPC)
// ─────────────────────────────────────────────────────────────────────────────

fn bench_shm_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("shm_hot_path");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100);

    // ── point_slot_set ────────────────────────────────────────────────────────
    {
        let slot = PointSlot::new();
        group.bench_function("point_slot_set", |b| {
            let mut ts = 1_748_000_000_000u64;
            b.iter(|| {
                ts = ts.wrapping_add(1);
                black_box(&slot).set(black_box(42.0), black_box(42.0), black_box(ts));
            });
        });
    }

    // ── point_slot_load_consistent ────────────────────────────────────────────
    {
        let slot = PointSlot::new();
        slot.set(123.4, 123.4, 1_748_000_000_000);
        group.bench_function("point_slot_load_consistent", |b| {
            b.iter(|| {
                black_box(black_box(&slot).load_consistent());
            });
        });
    }

    // ── point_slot_set_then_load ──────────────────────────────────────────────
    {
        let slot = PointSlot::new();
        group.bench_function("point_slot_set_then_load", |b| {
            let mut ts = 1_748_000_000_000u64;
            b.iter(|| {
                ts = ts.wrapping_add(1);
                black_box(&slot).set(black_box(99.0), black_box(99.0), black_box(ts));
                black_box(black_box(&slot).load_consistent());
            });
        });
    }

    // ── unified_writer_set_action ─────────────────────────────────────────────
    {
        let dir = tempdir().unwrap();
        let config = bench_config(dir.path());
        let channel_points = bench_channel_points();
        let writer = UnifiedWriter::create(&config, &channel_points).unwrap();

        group.bench_function("unified_writer_set_action", |b| {
            let mut ts = 1_748_000_000_000u64;
            b.iter(|| {
                ts = ts.wrapping_add(1);
                black_box(black_box(&writer).set_action(
                    black_box(1001),
                    black_box(2), // Control
                    black_box(0),
                    black_box(1.0),
                    black_box(ts),
                ));
            });
        });
    }

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Group 2: M2C dispatch — UDS loopback latency
// ─────────────────────────────────────────────────────────────────────────────

fn bench_m2c_uds(c: &mut Criterion) {
    let mut group = c.benchmark_group("m2c_uds");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(50);

    // Start a tokio runtime to host the UDS listener task and drive the notifier.
    let rt = Runtime::new().unwrap();

    // Bind a UDS listener in a tempdir, then spawn a reader task that reads
    // and discards every 48-byte message as fast as it arrives.
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("bench-m2c.sock");
    let sock_str = sock_path.to_str().unwrap().to_string();

    let listener = rt.block_on(async { tokio::net::UnixListener::bind(&sock_path).unwrap() });

    // Spawn the draining reader.
    rt.spawn(async move {
        let mut buf = [0u8; 64];
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                while stream.read(&mut buf).await.unwrap_or(0) != 0 {}
            });
        }
    });

    // Connect the notifier.
    let mut notifier = rt.block_on(async { ShmNotifier::connect(&sock_str).await.unwrap() });

    group.bench_function("shm_notifier_notify_uds_loopback", |b| {
        use voltage_model::PointType;
        use voltage_rtdb_shm::notifier::NotifyResult;
        let mut seq = 0u64;
        b.iter(|| {
            seq += 1;
            let result: NotifyResult = rt.block_on(async {
                black_box(&mut notifier)
                    .notify(
                        black_box(1001),
                        black_box(PointType::Control),
                        black_box(0),
                        black_box(1.0),
                        black_box(1_748_000_000_000),
                    )
                    .await
            });
            black_box(result);
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Group 3: End-to-end ShmDispatch (mock notifier — measures SHM write + Mutex
//          + generation check overhead; UDS kernel latency measured above)
// ─────────────────────────────────────────────────────────────────────────────

fn bench_shm_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("shm_dispatch");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100);

    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let config = bench_config(dir.path());
    let channel_points = bench_channel_points();
    let writer = Arc::new(UnifiedWriter::create(&config, &channel_points).unwrap());

    // Wire up a ShmDispatch with the real writer but a disabled notifier
    // (path = "" → NotifyResult::off(), no kernel UDS round-trip).
    let dispatch = ShmDispatch::new();
    dispatch.set_writer(Arc::clone(&writer), config.clone());
    let notifier = Arc::new(tokio::sync::Mutex::new(ShmNotifier::disabled()));
    dispatch.set_notifier(notifier);
    let dispatch = Arc::new(dispatch);

    let ctx = bench_route_ctx();

    group.bench_function("shm_dispatch_full_path", |b| {
        b.iter(|| {
            let outcome: DispatchOutcome = rt.block_on(async {
                black_box(&*dispatch)
                    .dispatch(black_box(&ctx), black_box(42.0))
                    .await
            });
            let _ = black_box(outcome);
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Group 4: OnChange tick Phase 0 — the paths PointWatch will eliminate
// ─────────────────────────────────────────────────────────────────────────────

use voltage_rules::{OnChangeState, PointKind, PointRef, ValueDeadband, should_trigger_onchange};

fn make_point_refs(n: usize) -> Vec<PointRef> {
    (0..n as u32)
        .map(|i| PointRef {
            instance: 1 + i / 10, // spread across a few instances
            point_type: PointKind::Measurement,
            point: i % 10,
        })
        .collect()
}

fn make_snapshot_map(refs: &[PointRef], value: f64) -> HashMap<String, Option<f64>> {
    refs.iter().map(|p| (p.cache_key(), Some(value))).collect()
}

fn make_seen_state(refs: &[PointRef], value: f64) -> OnChangeState {
    let mut state = OnChangeState::default();
    for p in refs {
        state.last_value.insert(p.cache_key(), value);
    }
    state
}

/// Populate a MemoryRtdb with `n` subscribed points spread across instances.
/// Returns the populated rtdb and the subscription set.
fn populate_memory_rtdb(n: usize) -> (Arc<MemoryRtdb>, HashSet<PointRef>) {
    use bytes::Bytes;
    use voltage_rtdb::KeySpaceConfig;

    let rt = Runtime::new().unwrap();
    let rtdb = Arc::new(MemoryRtdb::new());
    let keyspace = KeySpaceConfig::production_cached();

    let mut subs: HashSet<PointRef> = HashSet::new();
    for i in 0..n as u32 {
        let pref = PointRef {
            instance: 1 + i / 10,
            point_type: PointKind::Measurement,
            point: i % 10,
        };
        let hash_key = keyspace.instance_measurement_key(pref.instance);
        let field = pref.point.to_string();
        let value = Bytes::from(format!("{:.2}", 220.0 + i as f64 * 0.1));
        rt.block_on(async {
            rtdb.hash_set(&hash_key, &field, value).await.unwrap();
        });
        subs.insert(pref);
    }
    (rtdb, subs)
}

fn bench_onchange_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("onchange_tick_phase0");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(50);

    let rt = Runtime::new().unwrap();

    // ── onchange_phase0_memory_hmget (MemoryRtdb stand-in for Redis HMGET) ────
    for n in [10usize, 100, 1000] {
        let (rtdb, subs) = populate_memory_rtdb(n);
        let keyspace = voltage_model::KeySpaceConfig::production_cached();

        // Replicate the scheduler's fetch_point_snapshot() logic directly.
        // This lets us benchmark the exact shape (group-by-instance HMGET)
        // without needing to instantiate a full RuleScheduler.
        group.bench_with_input(BenchmarkId::new("phase0_hmget_memory", n), &n, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    use std::collections::HashMap as HM;
                    let mut grouped: HM<String, Vec<PointRef>> = HM::new();
                    for pref in &subs {
                        let hk = keyspace.instance_measurement_key(pref.instance);
                        grouped.entry(hk).or_default().push(*pref);
                    }
                    let mut out: HM<String, Option<f64>> = HM::with_capacity(subs.len());
                    for (hash_key, refs) in &grouped {
                        let fields: Vec<String> =
                            refs.iter().map(|p| p.point.to_string()).collect();
                        let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
                        let results = rtdb.hash_mget(hash_key, &field_refs).await.unwrap();
                        for (i, pref) in refs.iter().enumerate() {
                            let parsed = results
                                .get(i)
                                .and_then(|opt| opt.as_ref())
                                .and_then(|bytes| {
                                    std::str::from_utf8(bytes).ok()?.parse::<f64>().ok()
                                })
                                .filter(|v| v.is_finite());
                            out.insert(pref.cache_key(), parsed);
                        }
                    }
                    black_box(out)
                })
            });
        });
    }

    // ── should_trigger_onchange (pure CPU, no I/O) ────────────────────────────
    let vd = ValueDeadband::Absolute { threshold: 0.5 };
    let now = std::time::Instant::now();

    for n in [10usize, 100, 1000] {
        let refs = make_point_refs(n);

        // Scenario: value changed beyond deadband → triggers
        {
            let state = make_seen_state(&refs, 100.0);
            let snapshot = make_snapshot_map(&refs, 101.5);
            group.bench_with_input(
                BenchmarkId::new("should_trigger_value_changed", n),
                &n,
                |b, _| {
                    b.iter(|| {
                        black_box(should_trigger_onchange(
                            black_box(&state),
                            black_box(&refs),
                            black_box(None),
                            black_box(Some(&vd)),
                            black_box(&snapshot),
                            black_box(now),
                        ))
                    })
                },
            );
        }

        // Scenario: no trigger (full scan, deadband blocks)
        {
            let state = make_seen_state(&refs, 100.0);
            let snapshot = make_snapshot_map(&refs, 100.2);
            group.bench_with_input(
                BenchmarkId::new("should_trigger_no_change", n),
                &n,
                |b, _| {
                    b.iter(|| {
                        black_box(should_trigger_onchange(
                            black_box(&state),
                            black_box(&refs),
                            black_box(None),
                            black_box(Some(&vd)),
                            black_box(&snapshot),
                            black_box(now),
                        ))
                    })
                },
            );
        }
    }

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Criterion wiring
// ─────────────────────────────────────────────────────────────────────────────

fn configured() -> Criterion {
    Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(1))
}

criterion_group! {
    name = benches;
    config = configured();
    targets =
        bench_shm_hot_path,
        bench_m2c_uds,
        bench_shm_dispatch,
        bench_onchange_tick
}
criterion_main!(benches);
