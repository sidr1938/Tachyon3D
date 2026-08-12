use std::sync::atomic::{AtomicUsize, Ordering};

use tachyon3d_internal::app::AppT3D;
use tachyon3d_internal::schedule::executors::Executor;
use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
use tachyon3d_internal::systems::variadics::SystemMethods;
use tachyon3d_internal::workgroups::{Extensions, Strategy, WorkgroupHandler, Task};
use tachyon3d_macros::Resource;

static RUNS: AtomicUsize = AtomicUsize::new(0);

#[derive(Resource)]
struct Counter {
    value: u32,
}

#[derive(Resource)]
struct Other {
    value: u32,
}

fn tick(_c: &Other, _m: &mut Counter) {
    RUNS.fetch_add(1, Ordering::Relaxed);
}

fn tock(_c: &mut Other, _m: &mut Counter) {
    RUNS.fetch_add(1, Ordering::Relaxed);
}

#[test]
fn every_system_runs_once_per_frame() {
    struct Loop;
    struct Pool;

    let mut app = AppT3D::new();
    app.install(WorkgroupHandler::new())
        .add_schedule(Loop, Executor::MultiThreaded(MultiThreadedExecutor::new(Pool)))
        .add_workgroup(Pool, Strategy::WorkSteal, 4)
        .add_resource(Counter { value: 0 })
        .add_resource(Other { value: 0 })
        .add_systems(Loop,
                     (((tick, tick, tock), (tick), (tock, tick, tick)),
                      ((tick, tock, tick).order(), (tock), (tick, tick, tock))).order()
        );

    let systems = app.schedules.get_mut(Loop).unwrap().systems.len();
    app.schedules.get_mut(Loop).unwrap().cache_pointers(&mut app.resources);

    let frames = 25;
    for _ in 0..frames {
        app.schedules.get_mut(Loop).unwrap().run(&mut app.resources);
        app.full_sync();
    }
    app.full_shutdown();

    assert_eq!(RUNS.load(Ordering::Relaxed), systems * frames);
}

#[test]
fn tasks_run_their_payload() {
    static BOXED: AtomicUsize = AtomicUsize::new(0);
    static RAW: AtomicUsize = AtomicUsize::new(0);

    Task::new(|| {
        BOXED.fetch_add(1, Ordering::Relaxed);
    }).run();

    unsafe fn add(ctx: *mut (), payload: u64) {
        unsafe { (*(ctx as *mut AtomicUsize)).fetch_add(payload as usize, Ordering::Relaxed) };
    }
    unsafe { Task::raw(add, &RAW as *const AtomicUsize as *mut (), 7) }.run();

    assert_eq!(BOXED.load(Ordering::Relaxed), 1);
    assert_eq!(RAW.load(Ordering::Relaxed), 7);
}
