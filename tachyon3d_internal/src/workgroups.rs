use std::any::{Any, TypeId};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::{io, thread};
use std::cmp::{max, min};
use std::io::Write;
use std::thread::JoinHandle;
use std::time::Duration;
use fxhash::FxHashMap;
use crate::app::{AppT3D};
use crate::app::plugin::Installation;
use crossbeam_deque;
use crossbeam_deque::{Injector, Steal, Stealer, Worker};

pub struct Task(pub Box<dyn FnOnce() + Send>);
// T3D Default workgroup plugin
pub enum RequestThreads {
    Fixed(usize),
    Percent(u8),
    PercentMin(u8, usize),
}
#[derive(Debug)]
pub enum Strategy {
    WorkSteal,
    FiberWorkSteal,
    AsyncWorkSteal,
}

impl Installation for WorkgroupHandler {}

pub trait Extensions {
    fn add_workgroup<T: 'static>(&mut self, label: T, worker_type: Strategy, threads: usize) -> &mut Self;
    fn full_sync(&mut self) -> &mut Self;
    fn full_shutdown(&mut self, set: bool) -> &mut Self;
}

// The main thread can miss an unpark from a worker that died, so syncing polls instead of
// parking indefinitely
const SYNC_POLL: Duration = Duration::from_millis(1);

const MISSING_HANDLER: &str =
    "WORKGROUP: no WorkgroupHandler is installed, install it with app.install(WorkgroupHandler::new())";

impl Extensions for AppT3D {
    fn add_workgroup<T: 'static>(&mut self, label: T, worker_type: Strategy, threads: usize) -> &mut Self {
        self.resources.get_mut::<WorkgroupHandler>().expect(MISSING_HANDLER)
            .internal_add_workgroup(label, worker_type, threads);
        self
    }
    fn full_shutdown(&mut self, set: bool) -> &mut Self {
        let workgroup_handler = self.resources.get_mut::<WorkgroupHandler>().expect(MISSING_HANDLER);
        let mut failures = Vec::new();
        for (_, workgroup) in workgroup_handler.workgroups.iter_mut() {
            workgroup.shutdown.store(set, Ordering::Release);
            for worker in workgroup.thread_pool.iter_mut() {
                if let Some(worker) = worker.take() {
                    worker.thread().unpark();
                    if let Err(payload) = worker.join() {
                        failures.push(format!("worker thread panicked: {}", panic_message(payload.as_ref())));
                    }
                }
            }
            // Panics caught inside the workers, reported here rather than being lost
            failures.append(&mut workgroup.panics.take());
        }
        report_failures(&failures);
        self
    }
    fn full_sync(&mut self) -> &mut Self {
        let workgroup_handler = self.resources.get_mut::<WorkgroupHandler>().expect(MISSING_HANDLER);
        let mut failures = Vec::new();
        for (_, workgroup) in workgroup_handler.workgroups.iter_mut() {
            while workgroup.tasks.load(Ordering::Acquire) != 0 {
                thread::park_timeout(SYNC_POLL);
            }
            failures.append(&mut workgroup.panics.take());
        }
        report_failures(&failures);
        self
    }
}

fn report_failures(failures: &[String]) {
    if !failures.is_empty() {
        panic!("WORKGROUP: {} task(s) failed:\n{}", failures.len(), failures.join("\n"));
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("panicked with a payload that is not a string")
    }
}

// Panics raised by tasks running on worker threads, kept until the main thread syncs or shuts
// the workgroup down so they are not lost with the thread that hit them
#[derive(Debug, Default)]
pub struct PanicLog {
    inner: Mutex<Vec<String>>,
}

impl PanicLog {
    fn record(&self, message: String) {
        self.lock().push(message);
    }
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lock())
    }
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
    // A poisoned log still holds every panic recorded before the poisoning, so it is used as is
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

enum WorkerStatus {
    Ready = 0,
    Working = 1,
    Shutdown = 2,
}
pub struct WorkgroupHandler {
    pub workgroups: FxHashMap<TypeId, Workgroup>,
}
#[derive(Debug)]
pub struct Workgroup {
    pub thread_pool: Vec<Option<JoinHandle<()>>>,
    pub tasks: Arc<AtomicUsize>,
    pub shutdown: Arc<AtomicBool>,
    pub statuses: Arc<[AtomicU8]>,
    pub injector: Arc<Injector<Task>>,
    pub panics: Arc<PanicLog>,
}

impl WorkgroupHandler {
    pub fn new() -> WorkgroupHandler {
        WorkgroupHandler {
            workgroups: Default::default(),
        }
    }
    fn internal_add_workgroup<T: 'static>(&mut self, label: T, strategy: Strategy, threads: usize) {
        // Overwriting a workgroup drops its join handles, leaking threads that can never be
        // joined or shut down again
        assert!(
            !self.workgroups.contains_key(&label.type_id()),
            "WORKGROUP: a workgroup labelled `{}` already exists",
            std::any::type_name::<T>()
        );
        let mut workers = Vec::new();
        let mut stealers = Vec::new();
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut statuses = Vec::new();
        for _ in 0..threads {
            let worker = Worker::new_lifo();
            statuses.push(AtomicU8::from(WorkerStatus::Ready as u8));
            stealers.push(worker.stealer());
            workers.push(worker);
        }
        let statuses = Arc::from(statuses);
        let stealers = Arc::new(stealers);
        let injector = Default::default();
        let mut thread_pool = Vec::new();
        let group = self.workgroups.len();
        let tasks = Arc::new(AtomicUsize::new(0));
        let panics: Arc<PanicLog> = Default::default();
        let main_thread = std::thread::current();
        for (id, worker) in workers.into_iter().enumerate() {
            // ARC Shares
            let injector_arc = Arc::clone(&injector);
            let statuses_arc = Arc::clone(&statuses);
            let stealers_arc = Arc::clone(&stealers);
            let tasks_arc = Arc::clone(&tasks);
            let shutdown_arc = Arc::clone(&shutdown);
            let panics_arc = Arc::clone(&panics);
            let main_thread_clo = main_thread.clone();
            thread_pool.push(
                Some(std::thread::spawn(move || { worker_logic(injector_arc, worker, stealers_arc, statuses_arc, shutdown_arc, tasks_arc, panics_arc, main_thread_clo, id, group) })
                ));
        }
        self.workgroups.insert(label.type_id(), Workgroup {
            thread_pool,
            tasks,
            shutdown,
            statuses,
            injector,
            panics,
        });
    }
}

thread_local! {

}
// Runs a task without letting a panic in it take down the worker, since a dead worker never
// decrements the task counter, which leaves the main thread waiting on a sync that never completes
fn run_task(task: Task, tasks: &AtomicUsize, panics: &PanicLog, group: usize, id: usize) {
    let result = catch_unwind(AssertUnwindSafe(|| task.0()));
    tasks.fetch_sub(1, Ordering::Relaxed);
    if let Err(payload) = result {
        panics.record(format!("group {group} thread {id}: {}", panic_message(payload.as_ref())));
    }
}

fn worker_logic(queue: Arc<Injector<Task>>, worker: Worker<Task>, stealers: Arc<Vec<Stealer<Task>>>, statuses: Arc<[AtomicU8]>, shutdown: Arc<AtomicBool>, tasks: Arc<AtomicUsize>, panics: Arc<PanicLog>, main_thread: std::thread::Thread, id: usize, group: usize) {
    loop {
        statuses[id].store(1, Ordering::Relaxed);
        loop {
            //io::stdout().flush().unwrap();
            match worker.pop() {
                Some(task) => { run_task(task, &tasks, &panics, group, id); continue; },
                None => {}
            }
            match queue.steal_batch_with_limit_and_pop(&worker, max(1, queue.len() / 2)) {
                Steal::Success(task) => {
                    run_task(task, &tasks, &panics, group, id); continue; }
                Steal::Retry => { continue; }
                Steal::Empty => {}
            }
            // Finding victims
            let mut idx = rand::random_range(0..(stealers.len()));
            for _ in 0..stealers.len() {
                if !stealers[idx % stealers.len()].is_empty() {
                    break;
                }
                idx += 1;
            }
            let victim = &stealers[idx % stealers.len()];
            match victim.steal_batch_with_limit_and_pop(&worker, max(1, victim.len() / 2)) {
                Steal::Success( task) => {
                    run_task(task, &tasks, &panics, group, id); continue;
                },
                Steal::Retry => { continue; },
                Steal::Empty => {},
            }
            break;
        }
        if tasks.load(Ordering::Acquire) == 0 {
            if shutdown.load(Ordering::Relaxed) {
                //println!("GROUP {group} THREAD {id} SHUTDOWN");
                statuses[id].store(2, Ordering::Relaxed);
                //io::stdout().flush().unwrap();
                break;
            }
            //println!("GROUP {group} THREAD {id} PARKED");
            statuses[id].store(0, Ordering::Relaxed);
            main_thread.unpark();
            thread::park();
        }
    }
}
