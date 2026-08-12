use std::any::TypeId;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tachyon3d::app::AppT3D;
use tachyon3d::resources::fetch::{DisjointedAccess, FetchError};
use tachyon3d::schedule::executors::Executor;
use tachyon3d::schedule::executors::multi::MultiThreadedExecutor;
use tachyon3d::schedule::executors::single::SingleThreadedExecutor;
use tachyon3d::systems::variadics::OwnershipType;
use tachyon3d::workgroups::{Extensions, Strategy, WorkgroupHandler};
use tachyon3d::Resource;

#[derive(Resource)]
struct Counter {
    value: u32,
}

struct Loop;
struct Pool;

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&'static str>().map(|m| m.to_string()))
        .unwrap_or_default()
}

#[test]
fn fetch_args_reports_missing_and_aliased_resources() {
    let mut app = AppT3D::new();
    let counter = TypeId::of::<Counter>();

    assert_eq!(
        app.resources.internal.fetch_args(&vec![(OwnershipType::Ref, counter)]).err(),
        Some(FetchError::MissingResource(counter))
    );

    app.add_resource(Counter { value: 0 });
    assert_eq!(
        app.resources
            .internal
            .fetch_args(&vec![(OwnershipType::Ref, counter), (OwnershipType::Mut, counter)])
            .err(),
        Some(FetchError::AliasedResource(counter))
    );
    assert!(app.resources.internal.fetch_args(&vec![(OwnershipType::Mut, counter)]).is_ok());
}

#[test]
fn cache_pointers_reports_systems_asking_for_unknown_resources() {
    fn system(_: &mut Counter) {}

    let mut app = AppT3D::new();
    app.add_schedule(Loop, Executor::SingleThreaded(SingleThreadedExecutor))
        .add_systems(Loop, system);

    let error = app
        .schedules
        .get_mut(Loop)
        .unwrap()
        .cache_pointers(&mut app.resources)
        .map(|_| ())
        .expect_err("the resource was never added, so caching cannot succeed");
    assert_eq!(error, FetchError::MissingResource(TypeId::of::<Counter>()));
}

#[test]
fn a_panicking_system_does_not_strip_the_schedule_of_its_executor() {
    let mut app = AppT3D::new();
    app.add_schedule(Loop, Executor::SingleThreaded(SingleThreadedExecutor))
        .add_systems(Loop, || panic!("system blew up"));

    let payload = catch_unwind(AssertUnwindSafe(|| {
        app.schedules.get_mut(Loop).unwrap().run(&mut app.resources)
    }))
    .expect_err("the panic should reach the caller");
    assert_eq!(panic_message(payload), "system blew up");
    // Without this the next run would report "no executor" and hide the real failure
    assert!(app.schedules.get_mut(Loop).unwrap().executor.is_some());
}

#[test]
fn a_panicking_task_is_reported_on_sync_instead_of_hanging_the_workgroup() {
    let ran = Arc::new(AtomicUsize::new(0));
    let ran_in_system = Arc::clone(&ran);

    let mut app = AppT3D::new();
    app.install(WorkgroupHandler::new())
        .add_schedule(Loop, Executor::MultiThreaded(MultiThreadedExecutor::new(Pool)))
        .add_workgroup(Pool, Strategy::WorkSteal, 2)
        .add_systems(Loop, move || {
            ran_in_system.fetch_add(1, Ordering::Relaxed);
            panic!("task blew up");
        });

    app.schedules.get_mut(Loop).unwrap().run(&mut app.resources);
    let payload = catch_unwind(AssertUnwindSafe(|| {
        app.full_sync();
    }))
    .expect_err("the task panic should surface on the main thread");
    let message = panic_message(payload);
    assert!(message.contains("task blew up"), "unexpected panic message: {message}");
    assert_eq!(ran.load(Ordering::Relaxed), 1);

    // The workers survived the panicking task, so the workgroup still shuts down cleanly
    app.full_shutdown(true);
}

#[test]
fn adding_a_workgroup_twice_is_rejected_instead_of_leaking_its_threads() {
    let mut app = AppT3D::new();
    app.install(WorkgroupHandler::new())
        .add_workgroup(Pool, Strategy::WorkSteal, 1);

    let payload = catch_unwind(AssertUnwindSafe(|| {
        app.add_workgroup(Pool, Strategy::WorkSteal, 1);
    }))
    .expect_err("the duplicate workgroup should be rejected");
    assert!(panic_message(payload).contains("already exists"));

    app.full_shutdown(true);
}
