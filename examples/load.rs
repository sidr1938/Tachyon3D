// Heavy-system load test: 2 parallel readers + 2 chained writers over 1M elements.
// Usage: cargo run --release --example load [iters] [threads] [multi|single]
use tachyon3d::Resource;
use tachyon3d::app::{AppT3D, Plugin};
use tachyon3d::schedule::executors::Executor;
use tachyon3d::schedule::executors::multi::MultiThreadedExecutor;
use tachyon3d::systems::variadics::SystemMethods;
use tachyon3d::workgroups::{Extensions, Strategy, WorkgroupHandler};

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize, default: u64| -> u64 {
        args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default)
    };
    let iters = arg(1, 100) as usize;
    let threads = arg(2, 4) as usize;
    let multi = args.get(3).map(|s| s != "single").unwrap_or(true);
    let element_count = 1_000_000;

    let mut app = AppT3D::new();
    app.add_resource(ReadResource { multipliers: vec![1.5; element_count] });
    app.add_resource(WriteResource { positions: vec![0.0; element_count] });
    app.install(WorkgroupHandler::new())
        .add_workgroup(ComputeTP, Strategy::WorkSteal, threads);
    if multi {
        app.add_schedule(Update, Executor::MultiThreaded(MultiThreadedExecutor::new(ComputeTP)));
    } else {
        app.add_schedule(Update, Executor::default());
    }
    app.add_systems(
        Update,
        (
            system_read_1,
            system_read_2,
            (system_write_1, system_write_2).order(),
        ),
    );
    app.schedules.get_mut(Update).unwrap().cache_pointers(&mut app.resources);

    let start = std::time::Instant::now();
    for _ in 0..iters {
        app.schedules.get_mut(Update).unwrap().run(&mut app.resources);
        app.full_sync();
    }
    let elapsed = start.elapsed();
    app.full_shutdown();
    println!(
        "tachyon executor={} threads={} iters={} total={:?} per_iter={:.2}ms",
        if multi { "multi" } else { "single" },
        threads,
        iters,
        elapsed,
        elapsed.as_secs_f64() * 1000.0 / iters as f64
    );
}
