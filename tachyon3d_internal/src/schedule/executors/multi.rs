use std::any::{Any, TypeId};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use crossbeam_deque::Injector;
use slotmap::{DenseSlotMap, Key, KeyData};
use crate::ResourceHandler;
use crate::resources::fetch::DisjointedAccess;
use crate::schedule::executors::SendPointer;
use crate::schedule::graph::{DependencyGraph, SystemKey};
use crate::schedule::Schedule;
use crate::systems::entry::SystemEntry;
use crate::workgroups::{WorkgroupHandler, Task, push_task};

// Everything a queued system needs to run and to release its dependents.
// Boxed so the address stays stable while the executor itself is moved in and out of the schedule.
struct DispatchContext {
    systems: SendPointer<DenseSlotMap<SystemKey, SystemEntry>>,
    dep_graph: SendPointer<DependencyGraph>,
    resources: SendPointer<ResourceHandler>,
    queue: SendPointer<Arc<Injector<Task>>>,
}

pub struct MultiThreadedExecutor {
    workgroup: TypeId,
    context: Box<Option<DispatchContext>>,
}
impl MultiThreadedExecutor {
    pub fn execute(&mut self, schedule: &mut Schedule, resources: &mut ResourceHandler) {
        // Pointer creation
        let system_ptr = SendPointer::new_mut(&mut schedule.systems);
        let schedule_ptr = SendPointer::new(&schedule.dep_graph);
        let resource_ptr = SendPointer::new_mut(resources);

        let workgroup = resources.get_mut::<WorkgroupHandler>().unwrap().workgroups.get_mut(&self.workgroup).unwrap();
        let root = schedule.dep_graph.root.unwrap();
        // Pointer for the queue, needs access to workgroup which is in resources so we make the pointer for this after
        // to avoid two mutable borrows at the same time
        let queue_ptr = SendPointer::new(&workgroup.injector);
        *self.context = Some(DispatchContext {
            systems: system_ptr,
            dep_graph: schedule_ptr,
            resources: resource_ptr,
            queue: queue_ptr,
        });
        let context = (&raw mut *self.context) as *mut ();

        dispatch_system(context, root);
        for associate in schedule.dep_graph.edges.get(&root).unwrap().associates.iter() {
            dispatch_system(context, *associate);
        }
        // Ready the threads
        let mut threads = 0;
        workgroup.tasks.fetch_add(schedule.systems.len(), Ordering::Relaxed);
        for (idx, worker) in workgroup.thread_pool.iter_mut().enumerate() {
            if threads == schedule.systems.len() {
                break;
            }
            if let Some(worker) = worker {
                // Ready, or asleep because it ran out of reachable work during the last frame
                if matches!(workgroup.statuses[idx].load(Ordering::Acquire), 0 | 3) {
                    worker.thread().unpark();
                    threads += 1;
                }
            }
        }
    }
    pub fn new<T: 'static>(workgroup: T) -> Self {
        MultiThreadedExecutor {
            workgroup: workgroup.type_id(),
            context: Box::new(None),
        }
    }
}

fn dispatch_system(context: *mut (), node: SystemKey) {
    unsafe {
        let queue = (*(context as *mut Option<DispatchContext>)).as_ref().unwrap().queue;
        push_task(queue.as_ref(), Task::raw(run_system, context, node.data().as_ffi()));
    }
}

// The multithreaded version of the single threaded systems dispatcher
unsafe fn run_system(context: *mut (), payload: u64) {
    unsafe {
        let node = SystemKey::from(KeyData::from_ffi(payload));
        let ctx = (*(context as *mut Option<DispatchContext>)).as_ref().unwrap();
        // Pointer initialize for pointers converted more than once
        let systems = ctx.systems.as_mut();
        let dep_graph = ctx.dep_graph.as_ref();

        let entry = systems.get_mut(node).unwrap();
        if let Some(cache) = entry.cache.as_ref() {
            (entry.system)(cache);
        } else {
            let data = ctx.resources.as_mut().internal.fetch_args_unchecked(&entry.ptrs);
            (entry.system)(&data);
        }
        for dependent_node in dep_graph.edges.get(&node).unwrap().dependents.iter() {
            let max = systems.get(*dependent_node).unwrap().dependency_count;
            let count = &mut systems.get_mut(*dependent_node).unwrap().dependency_gate;
            let prev = count.fetch_add(1, Ordering::Acquire);
            if prev + 1 == max {
                for dep_associated_node in dep_graph.edges.get(dependent_node).unwrap().associates.iter().rev() {
                    dispatch_system(context, *dep_associated_node);
                }
                dispatch_system(context, *dependent_node);
                count.store(0, Ordering::Release);
            }
        }
    }
}
