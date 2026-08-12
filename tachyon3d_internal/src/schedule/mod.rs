use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::ops::DerefMut;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use fxhash::FxHashMap;
use slotmap::DenseSlotMap;
use crate::{ResourceHandler};
use crate::resources::fetch::{DisjointedAccess, FetchError};
use crate::schedule::executors::Executor;

use crate::schedule::graph::{DependencyGraph, SystemKey};
use crate::systems::entry::SystemEntry;
use crate::systems::variadics::Destructure;


pub mod graph;
pub mod executors;

pub struct ScheduleHandler {
    pub internal: FxHashMap<TypeId, Schedule>,
}

impl ScheduleHandler {
    pub fn get<T: 'static>(&self, label: T) -> Option<&Schedule> {
        self.internal.get(&label.type_id()).and_then(|f| Some(f))
    }
    pub fn get_mut<T: 'static>(&mut self, label: T) -> Option<&mut Schedule> {
        self.internal.get_mut(&label.type_id()).and_then(move |f| Some(f))
    }
}

pub struct Schedule {
    pub systems: DenseSlotMap<SystemKey, SystemEntry>,
    pub executor: Option<Executor>,
    pub dep_graph: DependencyGraph,
}

impl Schedule {
    pub fn new(executor: Option<Executor>) -> Self {
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
        // The executor is moved out so it can be driven while it mutates the schedule,
        // so a panicking system would otherwise leave the schedule executor-less and every
        // later run would report "no executor" instead of the real failure
        let mut exec = self.executor.take();
        let result = catch_unwind(AssertUnwindSafe(|| {
            match exec.as_mut().expect("SCHEDULE: Called run on schedule with no executor") {
                Executor::SingleThreaded(single_threaded_executor) => {
                    single_threaded_executor.execute(self, resources);
                },
                Executor::MultiThreaded(multi_threaded_executor) => {
                    multi_threaded_executor.execute(self, resources);
                },
                Executor::Custom(executor) => {
                    executor.deref_mut().execute(self, resources);
                },
            }
        }));
        self.executor = exec;
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }
    // Validates every system's arguments while caching them, so an aliased or missing
    // resource is reported here instead of turning into a dangling pointer at run time
    pub fn cache_pointers(&mut self, resources: &mut ResourceHandler) -> Result<&mut Self, FetchError> {
        for (_, entry) in self.systems.iter_mut() {
            let data = resources.internal.fetch_args(&entry.ptrs)?;
            entry.cache = Some(data);
        }
        Ok(self)
    }
    pub fn add_systems<U>(&mut self, systems: impl Destructure<U>) -> &mut Self {
        let mut current_trigger = self.dep_graph.root;
        let mut curr = Vec::new();
        if let Some(current_trig) = self.dep_graph.root {
            curr.push(current_trig);
        }
        systems.destructure(&mut Vec::new(), &mut curr, &mut current_trigger, self);
        self
    }
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