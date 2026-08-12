use std::any::{Any, TypeId};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::{io, thread};
use std::cmp::{max, min};
use std::io::Write;
use std::sync::OnceLock;
use std::thread::{JoinHandle, Thread};
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
    static LOCAL_WORKER: Cell<(*const Injector<Task>, *const Worker<Task>, *const Signals)> =
        const { Cell::new((std::ptr::null(), std::ptr::null(), std::ptr::null())) };
}

// Queue work for `injector`'s workgroup. Called from a thread of that workgroup this pushes onto
// the thread's own deque (uncontended, LIFO, stealable), otherwise onto the shared injector.
// Either way a sleeping thread is woken, since the work may not be reachable by the ones awake.
#[inline]
pub fn push_task(injector: &Injector<Task>, task: Task) {
    LOCAL_WORKER.with(|local| {
        let (owner, worker, signals) = local.get();
        if std::ptr::eq(owner, injector) {
            unsafe {
                (*worker).push(task);
                (*signals).wake_one();
            }
        } else {
            injector.push(task)
        }
    })
}

// How many fruitless passes over the queues a thread makes before it goes to sleep
const SPIN_ROUNDS: u32 = 64;

enum WorkerStatus {
    Ready = 0,
    Working = 1,
    Shutdown = 2,
    // Out of reachable work while the frame is still running, woken by `Signals::wake_one`
    Asleep = 3,
}

// Lets a thread that ran out of work sleep instead of burning a core the others need, without
// the work it can't see yet being stranded until the end of the frame.
#[derive(Debug)]
pub struct Signals {
    statuses: Arc<[AtomicU8]>,
    handles: Box<[OnceLock<Thread>]>,
    sleepers: AtomicUsize,
}

impl Signals {
    // Claim one sleeping thread and wake it. Unparking a thread that has not parked yet is fine,
    // the token is kept and its next park returns immediately
    #[inline]
    fn wake_one(&self) {
        if self.sleepers.load(Ordering::Relaxed) == 0 {
            return;
        }
        for (id, handle) in self.handles.iter().enumerate() {
            if self.statuses[id]
                .compare_exchange(
                    WorkerStatus::Asleep as u8,
                    WorkerStatus::Working as u8,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                self.sleepers.fetch_sub(1, Ordering::Relaxed);
                if let Some(handle) = handle.get() {
                    handle.unpark();
                }
                return;
            }
        }
    }
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
    pub signals: Arc<Signals>,
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
        let statuses: Arc<[AtomicU8]> = Arc::from(statuses);
        let signals = Arc::new(Signals {
            statuses: Arc::clone(&statuses),
            handles: (0..threads).map(|_| OnceLock::new()).collect(),
            sleepers: AtomicUsize::new(0),
        });
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
            let signals_arc = Arc::clone(&signals);
            thread_pool.push(
                Some(std::thread::spawn(move || { worker_logic(injector_arc, worker, stealers_arc, statuses_arc, shutdown_arc, tasks_arc, main_thread_clo, signals_arc, id, group) }))
            );
        }
        self.workgroups.insert(label.type_id(), Workgroup {
            thread_pool,
            tasks,
            shutdown,
            statuses,
            injector,
            signals,
        });
    }
}

fn worker_logic(queue: Arc<Injector<Task>>, worker: Worker<Task>, stealers: Arc<Vec<Stealer<Task>>>, statuses: Arc<[AtomicU8]>, shutdown: Arc<AtomicBool>, tasks: Arc<AtomicUsize>, main_thread: std::thread::Thread, signals: Arc<Signals>, id: usize, group: usize) {
    signals.handles[id].set(thread::current()).ok();
    LOCAL_WORKER.with(|local| local.set((Arc::as_ptr(&queue), &raw const worker, Arc::as_ptr(&signals))));
    let mut rng = 0x2545_f491_4f6c_dd1d_u64 ^ ((group as u64) << 32) ^ (id as u64 + 1);
    let mut empty_rounds = 0;
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
            empty_rounds = 0;
            // Whoever finishes the frame releases the main thread, not just thread 0, which may
            // well be asleep by this point
            main_thread.unpark();
            thread::park();
        } else if tasks_done > 0 || empty_rounds < SPIN_ROUNDS {
            // Work is still outstanding but none of it is reachable yet, so look again shortly
            empty_rounds = if tasks_done > 0 { 0 } else { empty_rounds + 1 };
            std::hint::spin_loop();
        } else {
            // The frame is still running but there is nothing this thread can reach. Spinning on
            // here just takes a core off the threads that do have work, so sleep until one of them
            // queues something; the executor picks this thread up again next frame either way
            empty_rounds = 0;
            signals.sleepers.fetch_add(1, Ordering::Relaxed);
            statuses[id].store(WorkerStatus::Asleep as u8, Ordering::Release);
            thread::park();
            if statuses[id]
                .compare_exchange(
                    WorkerStatus::Asleep as u8,
                    WorkerStatus::Working as u8,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                signals.sleepers.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}
