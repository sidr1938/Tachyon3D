// Tachyon3D API //
use crate::systems::executors::ExecutorMethods;
extern crate self as tachyon3d_internal;
use crate::systems::executors::Executor;
use systems::destructure::Destructure;
pub use crate::resources::ResourceHandler;
use std::ptr::NonNull;
use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU32;
use fxhash::{FxHashMap, FxHashSet};
use slotmap::{DefaultKey, DenseSlotMap};

#[cfg(feature = "hyper_fiber")]
pub mod hyper_fiber;
pub mod resources;
pub use resources::DisjointedAccess;
use crate::Executor::SingleThreaded;

pub mod systems;
// Notes //

/*
Minor-Medium
Arguments incorporated, derive resource proc macro, some more macros
Dynamic linking added
General cleanup in progress
Woking on hyper fiber
Added a wrapper system for future additions to arguments
Schedulers now have a run function, allows user control for scheduler execution rather
than plugins defining how your scheduler is executed (which is very bad)
Added scheduler tutorial
Added System ordering
Added graph_dot visualization
Added cache for system entries to drive a 90% perf boost in system dispatching
Optimized raw system dispatching for a 40% improvement in un-cached system dispatching
Added a dependency graph based scheduler builder
Added a dense slot map storage for systems
Minor-Minor:
Switched from backwards to forward tuple destructuring
Removals:
    Removed cache for resources, FxHashMap is pretty fast and honestly the micro optimization wasnt
    worth it since checks had to be done everytime for a cache for most methods
    Removed owned values
*/


// Things to add
/*
Async queues needed
Dynamic adding and removing of systems at runtime
Dynamic adding and removing of plugins at runtime
 */

// Optimizations to implenent
/*
Finished all optimization ideas for now.
*/

// qty Types //
// Make it so that resources actually require the Resource trait

// Pointers to resources for each argument
pub type ArgPointers<'a> = &'a [NonNull<u8>];

pub enum OwnershipType {
    Ref,
    Mut,
}

pub trait Resource {}
pub trait Unwrap { type INNER: Any; }
impl<T: Any + Resource + Send + Sync> Unwrap for T { type INNER = T; }

pub trait InnerAccess {
    const OWNERSHIP: (OwnershipType, TypeId);
    fn fetch_data(pointer: NonNull<u8>) -> Self;
}

// Runtime function
impl<T: Any + Unwrap + 'static > InnerAccess for &T {
    const OWNERSHIP: (OwnershipType, TypeId) = (OwnershipType::Ref, TypeId::of::<T::INNER>());
    // Called alot of times, best to inline this
    #[inline(always)]
    fn fetch_data(pointer: NonNull<u8>) -> Self {
        unsafe { return &*(pointer.as_ptr() as *const T) }
    }
}

// Runtime function
impl<T: Any + Unwrap + 'static> InnerAccess for &mut T {
    const OWNERSHIP: (OwnershipType, TypeId) = (OwnershipType::Mut, TypeId::of::<T::INNER>());
    // Same here
    #[inline(always)]
    fn fetch_data(pointer: NonNull<u8>) -> Self {
        unsafe { return &mut *(pointer.as_ptr() as *mut T) }
    }
}
pub trait RunAsOwned {
    fn runtime(self, app: AppT3D);
}


pub trait Plugin where Self: 'static {
    fn build(self, app: &mut AppT3D) where Self: Sized {
        app.resources.internal.insert(TypeId::of::<Self>(), Box::new(self));
    }
}

// In progress, unfinished, mainly to help with building plugins asynchronously
pub trait AsyncPlugin where Self: 'static {
    type Output: Send + 'static;
    fn build(self, app: &mut AppT3D) -> impl std::future::Future<Output = Self::Output> + Send;
}

pub struct ScheduleBuilder<'a> {
    schedule: &'a mut Schedule,
    current_trigger_node: Option<DefaultKey>,
    label: Option<String>,
}


pub struct Schedule {
    pub systems: DenseSlotMap<DefaultKey, SystemEntry>,
    pub executor: Option<Executor>,
    pub dep_graph: DependencyGraph,
}


impl Schedule {
    fn new(executor: Option<Executor>) -> Self {
        Schedule {
            systems: Default::default(),
            executor,
            dep_graph: DependencyGraph {
                edges: Default::default(),
                root: None,
            },
        }
    }
    pub fn run(&mut self, resources: &mut ResourceHandler) {
        let mut exec = self.executor.take();
        match exec.as_mut().expect("SCHEDULE: Called run on schedule with no executor") {
            Executor::SingleThreaded { exe } => {
                exe.execute(self, resources);
            },
            Executor::MultiThreaded { exe } => {
                exe.execute(self, resources);
            },
            Executor::Custom { exe } => {
                exe.deref_mut().execute(self, resources);
            },
        }
        self.executor = exec;
    }
    pub fn fetch_pointers(&mut self, resources: &mut ResourceHandler) {
        for (key, entry) in self.systems.iter_mut() {
            let data = resources.internal.get_disjoint_unchecked(&entry.ptrs);
            entry.cache = Some(data);
        }
    }
}

pub struct DependencyGraph {
    pub edges: FxHashMap<DefaultKey, NodeConnections>,
    pub root: Option<DefaultKey>
}
pub struct NodeConnections {
    pub dependents: Vec<DefaultKey>,
    pub associates: Vec<DefaultKey>
}
impl NodeConnections {
    fn new(dependents: Vec<DefaultKey>, associates: Vec<DefaultKey>) -> Self {
        NodeConnections {
            dependents,
            associates,
        }
    }
}

struct ResFfiVTable {
    size: usize,
    align: usize,
}


// Add stages


// ArgPointers has only pointers, lifetimes not needed
#[allow(dead_code)]
// Could possible extend this to more things later


pub trait Wrap<T> {
    fn wrap(data: T) -> Self;
}

impl<T: Any + Resource + Send + Sync> Wrap<T> for T {
    fn wrap(data: T) -> Self {
        data
    }
}

// Translation current_rank between internal storage and external input


// IN WORK

// Removes metadata, the struct will be the exact same size as the data it represents, making
// it a "transparent" wrapper allowing us to do safe type conversions via pointers
#[repr(transparent)]
pub struct NonSend<T: Any + 'static> {
    inner: T,
}
// For explicitly defined syntax, i considered a couple of options quite thoroughly
/*
1) mut foo: WrapperMut<Inner>, foo: Wrapper<Inner>, foo: WrapperOwned<Inner>
2) mut foo: Wrapper<&mut Inner>, foo: Wrapper<&Inner>, foo: Wrapper<Inner>
3) foo: &mut Wrapper<Inner>, foo: &Wrapper<Inner>, foo: Wrapper<Inner> (eventually)
*/

/*
Option 3 seemed the best in terms of cooperation with the already existing
syntax for resources which is just &Inner, &mut Inner (no wrapper needed)
and it looked idiomatic/nice enough while avoiding too much verbosity or wrappers
*/
impl<T> Deref for NonSend<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for NonSend<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct SystemEntry {
    pub system: Box<dyn FnMut(ArgPointers)>,
    pub label: Option<String>,
    pub ptrs: Vec<(OwnershipType, TypeId)>,
    pub cache: Option<Vec<NonNull<u8>>>,
    pub thread_safe: bool,
    pub dependency_gate: AtomicU32,
    pub dependency_count: u32,
}

struct AsyncQueueHandler {
    internal: FxHashMap<TypeId, Box<dyn Any>>,
}
pub struct SchedulerHandler {
    pub internal: FxHashMap<TypeId, Schedule>,
}

impl SchedulerHandler {
    pub fn get<T: 'static>(&self, label: T) -> Option<&Schedule> {
        self.internal.get(&label.type_id()).and_then(|f| Some(f))
    }
    pub fn get_mut<T: 'static>(&mut self, label: T) -> Option<&mut Schedule> {
        self.internal.get_mut(&label.type_id()).and_then(move |f| Some(f))
    }
}
pub struct AppT3D {
    // INP
    pub async_queue: AsyncQueueHandler,
    // Stable
    pub schedulers: SchedulerHandler,
    pub resources: ResourceHandler
}

impl AppT3D {
    pub fn new() -> Self {
        Self {
            async_queue: AsyncQueueHandler {
                internal: Default::default(),
            },
            schedulers: SchedulerHandler {
                internal: FxHashMap::default(),
            },
            resources: ResourceHandler {
                internal: FxHashMap::default(),
                ffi_internal: FxHashMap::default()
            },
        }
    }
    // Not really a full 'ecs' system and not sure if i want to go with an integrated ecs
    pub fn add_systems<T: 'static, U>(&mut self, schedule: T, systems: impl Destructure<U>) -> &mut Self {
        let scheduler = self.schedulers.internal.get_mut(&schedule.type_id()).expect("No schedular exists");
        let mut current_trigger = None;
        systems.destructure(&mut Vec::new(), &mut Vec::new(), &mut current_trigger, scheduler);
        self
    }
    pub fn add_scheduler<T: 'static>(&mut self, label: T, executor: Executor) -> &mut Self {
        self.schedulers.internal.insert(label.type_id(), Schedule::new(Some(executor)));
        self
    }
    // External functionality just like how bevy does it
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        plugin.build(self);
        self
    }
    // Convenience method
    pub fn insert_res<T: Any + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.resources.internal.insert(TypeId::of::<T>(), Box::new(resource));
        self
    }
    // Kind of a convenience method
    pub fn run_owned<R: RunAsOwned + Any>(mut self) {
        self.resources.remove::<R>().unwrap().runtime(self);
    }
}

impl Schedule {
    pub fn edges_to_dot(&self) -> String {
        let mut dot = String::from("digraph DependencyGraph {\n");
        let mut associated = HashSet::new();
        let mut last_rank_set = HashSet::new();
        dot.push_str("node [shape=box, style=\"rounded, filled\", width=1.6, fontname=\"Helvetica\", fillcolor=lightblue color=none];\n");
        dot.push_str("edge [color=cadetblue, style=dashed, penwidth=2.5];\n");
        for i in self.dep_graph.edges.iter() {
            for k in i.1.dependents.iter() {
                last_rank_set.insert(k);
            }
            for k in i.1.associates.iter() {
                associated.insert(k);
            }
        }
        //dot.push_str("node [fillcolor=lightblue, style=filled, color=none]\n");
        for i in self.dep_graph.edges.iter() {
            if associated.contains(i.0) {
                continue;
            }
            if i.1.dependents.is_empty() && !last_rank_set.contains(i.0) {
                dot.push_str(
                    &format!("{:?}", &self.systems.get(*i.0).unwrap().label.clone()
                        .unwrap_or(format!("{:?}", i.0)))
                );
            }
            for k in i.1.dependents.iter() {
                dot.push_str(
                    &format!("{:?} -> {:?}",
                             &self.systems.get(*i.0).unwrap().label.clone().unwrap_or(format!("{:?}", i.0)),
                             &self.systems.get(*k).unwrap().label.clone().unwrap_or(format!("{:?}", k)))
                );
            }
        }
        dot.push_str("node [shape=box, style=\"rounded, dashed\", fillcolor=grey, color=grey]\n");
        dot.push_str("edge [color=lightblue, style=dashed, penwidth=2, weight=1];\n");
        for i in self.dep_graph.edges.iter() {
            if associated.contains(i.0) {
                for k in i.1.dependents.iter() {
                    dot.push_str(
                        &format!("{:?} -> {:?}\n",
                                 &self.systems.get(*i.0).unwrap().label.clone().unwrap_or(format!("{:?}", i.0)),
                                 &self.systems.get(*k).unwrap().label.clone().unwrap_or(format!("{:?}", k)))
                    );
                }
                continue;
            }
            dot.push_str("{rank=same;\n");
            dot.push_str(&format!("{:?}; ",&self.systems.get(*i.0).unwrap().label.clone().unwrap_or(format!("{:?}", i.0))));
            for k in i.1.associates.iter() {
                associated.insert(k);
                dot.push_str(
                    &format!("{:?}; ", &self.systems.get(*k).unwrap().label.clone().unwrap_or(format!("{:?}", k)))
                );
                if self.dep_graph.edges.get(k).unwrap().dependents.is_empty() {
                    dot.push_str(
                        &format!("{:?} -> {:?} [style=invis, weight=0.25]\n",
                                 &self.systems.get(*i.0).unwrap().label.clone().unwrap_or(format!("{:?}", i.0)),
                                 &self.systems.get(*k).unwrap().label.clone().unwrap_or(format!("{:?}", k)))
                    );
                } else {
                    dot.push_str(
                        &format!("{:?} -> {:?} [style=invis, weight=0.75]\n",
                                 &self.systems.get(*i.0).unwrap().label.clone().unwrap_or(format!("{:?}", i.0)),
                                 &self.systems.get(*k).unwrap().label.clone().unwrap_or(format!("{:?}", k)))
                    );
                }

            }
            dot.push_str("}\n");
        }
        dot.push_str("}\n");
        dot
    }
}




