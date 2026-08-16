use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use tachyon3d::*;
use tachyon3d_internal::app::AppT3D;
use tachyon3d_internal::schedule::executors::Executor;
use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
use tachyon3d_internal::systems::variadics::SystemMethods;
use tachyon3d_internal::workgroups::{Extensions, Strategy, WorkgroupHandler};

#[derive(Resource)]
pub struct ReadResource {
    pub multipliers: Vec<f32>,
}

#[derive(Resource)]
pub struct WriteResource {
    pub positions: Vec<f32>,
}

struct ComputeTP;
struct Update;

// Systems
fn system_read_1(read: &ReadResource) {
    let sum: f32 = read.multipliers.iter().map(|x| x.sin().cos()).sum();
    std::hint::black_box(sum);
}

fn system_read_2(read: &ReadResource) {
    let sum: f32 = read.multipliers.iter().map(|x| x.tan().abs()).sum();
    std::hint::black_box(sum);
}

fn system_write_1(read: &ReadResource, write: &mut WriteResource) {
    for (p, m) in write.positions.iter_mut().zip(read.multipliers.iter()) {
        *p += m * 0.016;
    }
}

fn system_write_2(write: &mut WriteResource) {
    for p in write.positions.iter_mut() {
        *p *= 1.0001;
    }
}

// Helper to construct and populate the App
fn setup_app(element_count: usize) -> AppT3D {
    let mut app = AppT3D::new();
    app.add_resource(ReadResource {
        multipliers: vec![1.5; element_count],
    });
    app.add_resource(WriteResource {
        positions: vec![0.0; element_count],
    });
    app.install(WorkgroupHandler::new())
        .add_workgroup(ComputeTP, Strategy::WorkSteal, 5)
        .add_schedule(
            Update,
            Executor::MultiThreaded(MultiThreadedExecutor::new(ComputeTP)),
        )
        .cache_inputs(Update)
        .add_systems(
            Update,
            (
                system_read_1,
                system_read_2,
                (system_write_1, system_write_2).order(),
            ),
        );
    app
}

// -----------------------------------------------------------------------------
// Benchmark 1: Measures per-frame tick latency (Standard ECS Benchmark)
// -----------------------------------------------------------------------------
fn bench_tachyon3d_single_tick(c: &mut Criterion) {
    let element_count = 1_000_000;
    let mut app = setup_app(element_count);

    c.bench_function("tachyon3d_single_frame_tick", |b| {
        b.iter(|| {
            app.run(Update).full_sync();
        });
    });

    app.full_shutdown();
}

// -----------------------------------------------------------------------------
// Benchmark 2: Measures 100-frame batch throughput (Matching your test)
// -----------------------------------------------------------------------------
use std::time::Duration;

fn bench_tachyon3d_100_frame_batch(c: &mut Criterion) {
    let element_count = 1_000_000;

    let mut group = c.benchmark_group("tachyon3d_heavy_suite");

    // Reduce sample count (minimum allowed by Criterion is 10)
    group.sample_size(20);

    // Limit maximum total measurement time (e.g., 3 seconds)
    group.measurement_time(Duration::from_secs(3));

    // Optional: reduce warm-up time if startup overhead is high
    group.warm_up_time(Duration::from_secs(1));

    group.bench_function("tachyon3d_100_frames_batch", |b| {
        b.iter_batched(
            || setup_app(element_count),
            |mut app| {
                for _ in 0..100 {
                    app.run(Update).full_sync();
                }
                app.full_shutdown();
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tachyon3d_single_tick,
    bench_tachyon3d_100_frame_batch
);
criterion_main!(benches);