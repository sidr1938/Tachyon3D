use std::any::{Any, TypeId};
use std::cmp::min;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use crossbeam_deque::Injector;
use slotmap::DenseSlotMap;
use crate::ResourceHandler;
use crate::resources::fetch::DisjointedAccess;
use crate::schedule::executors::SendPointer;
use crate::schedule::graph::{DependencyGraph, NodeConnections, SystemKey};
use crate::schedule::Schedule;
use crate::systems::entry::SystemEntry;
use crate::workgroups::{WorkgroupHandler, Workgroup, Task};

pub struct MultiThreadedExecutor {
    workgroup: TypeId
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
        dispatch_system(queue_ptr, system_ptr, schedule_ptr, resource_ptr, root);
        for associate in schedule.dep_graph.edges.get(&root).unwrap().associates.iter() {
            dispatch_system(queue_ptr, system_ptr, schedule_ptr, resource_ptr, *associate);
        }

        // Ready the threads
        let mut threads = 0;

        workgroup.tasks.fetch_add(schedule.systems.len(), Ordering::Release);
        for (idx, worker) in workgroup.thread_pool.iter_mut().enumerate() {
            if threads == schedule.systems.len() {
                break;
            }
            if let Some(worker) = worker {
                if workgroup.statuses[idx].load(Ordering::Acquire) == 0 {
                    worker.thread().unpark();
                    threads += 1;
                }
            }
        }
    }
    pub fn new<T: 'static>(workgroup: T) -> Self {
        MultiThreadedExecutor {
            workgroup: workgroup.type_id()
        }
    }
}


fn dispatch_system(queue_ptr: SendPointer<Arc<Injector<Task>>>, systems_ptr: SendPointer<DenseSlotMap<SystemKey, SystemEntry>>, dep_graph_ptr: SendPointer<DependencyGraph>, resources_ptr: SendPointer<ResourceHandler>, node: SystemKey) {
    unsafe {
        queue_ptr.as_ref().push(Task(Box::new(move || {
            // The multithreaded version of the single threaded systems dispatcher
            // Pointer initialize for pointers converted more than once
            let systems = systems_ptr.as_mut();
            let dep_graph = dep_graph_ptr.as_ref();

            let entry = systems.get_mut(node).unwrap();
            if let Some(cache) = entry.cache.as_ref() {
                (entry.system)(cache);
            } else {
                let data = resources_ptr.as_mut().internal.fetch_args(&entry.ptrs)
                    .expect("SCHEDULE: System requests a missing resource or the same resource twice");
                (entry.system)(&data);
            }
            for dependent_node in dep_graph.edges.get(&node).unwrap().dependents.iter() {
                let max = systems.get(*dependent_node).unwrap().dependency_count;
                let count = &mut systems.get_mut(*dependent_node).unwrap().dependency_gate;
                let prev = count.fetch_add(1, Ordering::Acquire);
                if prev + 1 == max {
                    for dep_associated_node in dep_graph.edges.get(dependent_node).unwrap().associates.iter().rev() {
                        dispatch_system(queue_ptr, systems_ptr, dep_graph_ptr, resources_ptr, *dep_associated_node);
                    }
                    dispatch_system(queue_ptr, systems_ptr, dep_graph_ptr, resources_ptr, *dependent_node);
                    count.store(0, Ordering::Release);
                }
            }
        })));
    }

}