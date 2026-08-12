use std::any::{Any, TypeId};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;
use std::{io, thread};
use std::cmp::{max, min};
use std::io::Write;
use std::thread::JoinHandle;
use rustc_hash::FxHashMap;
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

impl Extensions for AppT3D {
    fn add_workgroup<T: 'static>(&mut self, label: T, worker_type: Strategy, threads: usize) -> &mut Self {
        self.resources.get_mut::<WorkgroupHandler>().unwrap()
            .internal_add_workgroup(label, worker_type, threads);
        self
    }
    fn full_shutdown(&mut self, set: bool) -> &mut Self {
        let workgroup_handler = self.resources.get_mut::<WorkgroupHandler>().unwrap();
        for (_, workgroup) in workgroup_handler.workgroups.iter_mut() {
            workgroup.shutdown.store(set, Ordering::Release);
            for worker in workgroup.thread_pool.iter_mut() {
                if let Some(worker) = worker.take() {
                    worker.thread().unpark();
                    worker.join().expect("Error");
                }
            }
        }
        self
    }
    fn full_sync(&mut self) -> &mut Self {
        let workgroup_handler = self.resources.get_mut::<WorkgroupHandler>().unwrap();
        for (_, workgroup) in workgroup_handler.workgroups.iter_mut() {
            // A timeout is required, an unpark that lands before this thread parks would
            // otherwise be lost and the sync would never complete
            while workgroup.tasks.load(Ordering::Acquire) != 0 {
                thread::park_timeout(Duration::from_millis(1));
            }
        }
        self
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
    pub injector: Arc<Injector<Task>>
}

impl WorkgroupHandler {
    pub fn new() -> WorkgroupHandler {
        WorkgroupHandler {
            workgroups: Default::default(),
        }
    }
    fn internal_add_workgroup<T: 'static>(&mut self, label: T, strategy: Strategy, threads: usize) {
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
        let main_thread = std::thread::current();
        for (id, worker) in workers.into_iter().enumerate() {
            // ARC Shares
            let injector_arc = Arc::clone(&injector);
            let statuses_arc = Arc::clone(&statuses);
            let stealers_arc = Arc::clone(&stealers);
            let tasks_arc = Arc::clone(&tasks);
            let shutdown_arc = Arc::clone(&shutdown);
            let main_thread_clo = main_thread.clone();
            thread_pool.push(
                Some(std::thread::spawn(move || { worker_logic(injector_arc, worker, stealers_arc, statuses_arc, shutdown_arc, tasks_arc, main_thread_clo, id, group) })
                ));
        }
        self.workgroups.insert(label.type_id(), Workgroup {
            thread_pool,
            tasks,
            shutdown,
            statuses,
            injector,
        });
    }
}

thread_local! {

}
// A task that panics must still be counted as finished, otherwise the task counter never
// reaches zero and every following sync or shutdown blocks forever
fn run_task(task: Task, tasks: &AtomicUsize) {
    let _ = catch_unwind(AssertUnwindSafe(move || task.0()));
    tasks.fetch_sub(1, Ordering::Relaxed);
}

fn worker_logic(queue: Arc<Injector<Task>>, worker: Worker<Task>, stealers: Arc<Vec<Stealer<Task>>>, statuses: Arc<[AtomicU8]>, shutdown: Arc<AtomicBool>, tasks: Arc<AtomicUsize>, main_thread: std::thread::Thread, id: usize, group: usize) {
    loop {
        statuses[id].store(1, Ordering::Relaxed);
        loop {
            //io::stdout().flush().unwrap();
            match worker.pop() {
                Some(task) => { run_task(task, &tasks); continue; },
                None => {}
            }
            match queue.steal_batch_with_limit_and_pop(&worker, max(1, queue.len() / 2)) {
                Steal::Success(task) => {
                    run_task(task, &tasks); continue; }
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
                    run_task(task, &tasks); continue;
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
