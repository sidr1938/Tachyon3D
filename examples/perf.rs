// Harness for measuring schedule dispatch overhead: 8 ordered stages of 17 systems each.
// Usage: cargo run --release --example perf [spin_iters] [frames] [threads] [multi|single]
// CPU time reporting reads /proc, so that line is Linux only.
use std::time::Instant;

use tachyon3d::app::{AppT3D, Plugin};
use tachyon3d::schedule::executors::Executor;
use tachyon3d::schedule::executors::multi::MultiThreadedExecutor;
use tachyon3d::systems::variadics::SystemMethods;
use tachyon3d::workgroups::{Extensions, Strategy, WorkgroupHandler};
use tachyon3d::Resource;

#[derive(Resource)]
struct Spin {
    iters: u64,
}

#[derive(Resource)]
struct Sink {
    value: u64,
}

struct ComputeTaskPool;
struct BasicLoop;

fn work(spin: &Spin, sink: &mut Sink) {
    let mut acc = 0u64;
    for i in 0..spin.iters {
        acc = acc.wrapping_add(i ^ acc);
    }
    sink.value = sink.value.wrapping_add(acc);
}

macro_rules! stage {
    () => {
        (
            work, work, work, work, work, work, work, work, work, work, work, work, work, work,
            work, work, work,
        )
    };
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize, default: u64| -> u64 {
        args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default)
    };
    let spin_iters = arg(1, 0);
    let frames = arg(2, 1000) as usize;
    let threads = arg(3, std::thread::available_parallelism().unwrap().get() as u64) as usize;
    let multi = args.get(4).map(|s| s != "single").unwrap_or(true);

    let mut app = AppT3D::new();
    app.install(WorkgroupHandler::new());
    if multi {
        app.add_schedule(
            BasicLoop,
            Executor::MultiThreaded(MultiThreadedExecutor::new(ComputeTaskPool)),
        );
    } else {
        app.add_schedule(BasicLoop, Executor::default());
    }
    app.add_workgroup(ComputeTaskPool, Strategy::WorkSteal, threads)
        .add_resource(Spin { iters: spin_iters })
        .add_resource(Sink { value: 0 })
        .add_systems(
            BasicLoop,
            (
                stage!(),
                stage!(),
                stage!(),
                stage!(),
                stage!(),
                stage!(),
                stage!(),
                stage!(),
            )
                .order(),
        );

    let system_count = app.schedules.get_mut(BasicLoop).unwrap().systems.len();
    app.schedules
        .get_mut(BasicLoop)
        .unwrap()
        .cache_pointers(&mut app.resources);

    // Warmup
    for _ in 0..50 {
        app.schedules.get_mut(BasicLoop).unwrap().run(&mut app.resources);
        app.full_sync();
    }

    let cpu_start = cpu_time_ns();
    let mut samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        let start = Instant::now();
        app.schedules.get_mut(BasicLoop).unwrap().run(&mut app.resources);
        app.full_sync();
        samples.push(start.elapsed().as_nanos() as u64);
    }
    let cpu = cpu_time_ns() - cpu_start;
    app.full_shutdown();

    samples.sort_unstable();
    let total: u64 = samples.iter().sum();
    let mean = total / samples.len() as u64;
    let p50 = samples[samples.len() / 2];
    let p99 = samples[samples.len() * 99 / 100];
    println!(
        "executor={} systems={} threads={} spin={} frames={} mean={:.1}us p50={:.1}us p99={:.1}us per_system={:.0}ns",
        if multi { "multi" } else { "single" },
        system_count,
        threads,
        spin_iters,
        frames,
        mean as f64 / 1000.0,
        p50 as f64 / 1000.0,
        p99 as f64 / 1000.0,
        mean as f64 / system_count as f64,
    );
    println!(
        "    cpu_per_frame={:.1}us cpu_per_system={:.0}ns",
        cpu as f64 / frames as f64 / 1000.0,
        cpu as f64 / frames as f64 / system_count as f64,
    );
}

// Whole-process CPU time (all worker threads), so spinning and contention show up
fn cpu_time_ns() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let tail = &stat[stat.rfind(')').unwrap() + 2..];
    let fields: Vec<&str> = tail.split_whitespace().collect();
    let ticks: u64 = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    ticks * (1_000_000_000 / 100)
}
