use std::any::{Any, TypeId};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::{io, thread};
use std::cmp::{max, min};
use std::io::Write;
use std::thread::JoinHandle;
use fxhash::FxHashMap;
use crate::app::{AppT3D};
use crate::app::plugin::Installation;
use crossbeam_deque;
use crossbeam_deque::{Injector, Steal, Stealer, Worker};

// A unit of work for a workgroup thread.
// `Raw` carries a bare fn pointer plus an opaque context, so dispatching a system costs no
// allocation; `Boxed` stays available for arbitrary closures.
pub enum Task {
    Raw {
        call: unsafe fn(*mut (), u64),
        ctx: *mut (),
        payload: u64,
    },
    Boxed(Box<dyn FnOnce() + Send>),
}

unsafe impl Send for Task {}

impl Task {
    pub fn new(closure: impl FnOnce() + Send + 'static) -> Self {
        Task::Boxed(Box::new(closure))
    }
    // Safety: `ctx` must stay valid and correctly typed for `call` until the task runs
    pub unsafe fn raw(call: unsafe fn(*mut (), u64), ctx: *mut (), payload: u64) -> Self {
        Task::Raw { call, ctx, payload }
    }
    #[inline(always)]
    pub fn run(self) {
        match self {
            Task::Raw { call, ctx, payload } => unsafe { call(ctx, payload) },
            Task::Boxed(closure) => closure(),
        }
    }
}
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
    fn full_shutdown(&mut self) -> &mut Self;
}

impl Extensions for AppT3D {
    fn add_workgroup<T: 'static>(&mut self, label: T, worker_type: Strategy, threads: usize) -> &mut Self {
        self.resources.get_mut::<WorkgroupHandler>().unwrap()
            .internal_add_workgroup(label, worker_type, threads);
        self
    }
    fn full_shutdown(&mut self) -> &mut Self {
        let workgroup_handler = self.resources.get_mut::<WorkgroupHandler>().unwrap();
        for (_, workgroup) in workgroup_handler.workgroups.iter_mut() {
            workgroup.shutdown.store(true, Ordering::Release);
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
            while workgroup.tasks.load(Ordering::Acquire) != 0 {
                thread::park();
                // for status in workgroup.statuses.iter() {
                //     while !status.load(Ordering::Acquire) == 0 {
                //         std::hint::spin_loop();
                //     }
                // }
            }
        }
        self
    }
}

thread_local! {
    // The deque owned by this workgroup thread, so a task can queue its follow-up work locally
    // instead of going back through the shared injector
    static LOCAL_WORKER: Cell<(*const Injector<Task>, *const Worker<Task>)> =
        const { Cell::new((std::ptr::null(), std::ptr::null())) };
}

// Queue work for `injector`'s workgroup. Called from a thread of that workgroup this pushes onto
// the thread's own deque (uncontended, LIFO, stealable), otherwise onto the shared injector.
#[inline]
pub fn push_task(injector: &Injector<Task>, task: Task) {
    LOCAL_WORKER.with(|local| {
        let (owner, worker) = local.get();
        if std::ptr::eq(owner, injector) {
            unsafe { (*worker).push(task) }
        } else {
            injector.push(task)
        }
    })
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
                Some(std::thread::spawn(move || { worker_logic(injector_arc, worker, stealers_arc, statuses_arc, shutdown_arc, tasks_arc, main_thread_clo, id, group) }))
            );
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

fn worker_logic(queue: Arc<Injector<Task>>, worker: Worker<Task>, stealers: Arc<Vec<Stealer<Task>>>, statuses: Arc<[AtomicU8]>, shutdown: Arc<AtomicBool>, tasks: Arc<AtomicUsize>, main_thread: std::thread::Thread, id: usize, group: usize) {
    LOCAL_WORKER.with(|local| local.set((Arc::as_ptr(&queue), &raw const worker)));
    let mut rng = 0x2545_f491_4f6c_dd1d_u64 ^ ((group as u64) << 32) ^ (id as u64 + 1);
    loop {
        statuses[id].store(1, Ordering::Relaxed);
        let mut tasks_done = 0;
        loop {
            //io::stdout().flush().unwrap();
            match worker.pop() {
                Some(task) => {
                    task.run();
                    tasks_done +=1;
                    continue;
                },
                None => {}
            }
            if tasks.load(Ordering::Acquire) == 0 {
                break;
            }
            match queue.steal_batch_with_limit_and_pop(&worker, max(1, queue.len() / 2)) {
                Steal::Success(task) => {
                    task.run();
                    tasks_done +=1;
                    continue;
                }
                Steal::Retry => { continue; }
                Steal::Empty => {}
            }
            if tasks.load(Ordering::Acquire) == 0 {
                break;
            }
            // Finding victims, starting from a random one so the threads don't all pile onto the
            // same deque. Skips this thread's own deque and any victim that is already empty
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let start = rng as usize % stealers.len();
            let mut retry = false;
            for offset in 0..stealers.len() {
                let idx = (start + offset) % stealers.len();
                if idx == id || stealers[idx].is_empty() {
                    continue;
                }
                let victim = &stealers[idx];
                match victim.steal_batch_with_limit_and_pop(&worker, max(1, victim.len() / 2)) {
                    Steal::Success( task ) => {
                        task.run();
                        tasks_done +=1;
                        retry = true;
                        break;
                    },
                    Steal::Retry => { retry = true; break; },
                    Steal::Empty => {},
                }
            }
            if retry {
                continue;
            }
            break;
        }

        let prev = tasks.fetch_sub(tasks_done, Ordering::AcqRel);
        if prev - tasks_done == 0 {
            if shutdown.load(Ordering::Relaxed) {
                statuses[id].store(2, Ordering::Relaxed);
                break;
            }
            statuses[id].store(0, Ordering::Relaxed);
            if id == 0 {
                main_thread.unpark();
            }
            thread::park();
        } else {
            std::hint::spin_loop();
        }
    }
}
