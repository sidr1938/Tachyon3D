
pub mod macros;

use std::any::{Any, TypeId};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use rustc_hash::{FxHashMap, FxHashSet};
use crate::resources::access::Unwrap;
use crate::schedule::{Schedule};
use crate::schedule::graph::{NodeConnections, SystemKey};
use crate::systems::entry::{SendSyncNonNull, SystemEntry};

pub trait Destructure<T> {
    fn destructure<'a>(self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>);

}


pub struct Sequence<T>(T);
pub trait SystemMethods<T> {
    fn order(self) -> Sequence<Self> where Self: Sized;
}

impl<F> Destructure<()> for F where F: FnMut() + 'static + Send + Sync {
    // Conversion
    fn destructure<'a>(mut self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
        let key = schedule.systems.insert(SystemEntry {
            label: None,
            system: Box::new(
                move |_: &[SendSyncNonNull]| {
                    self();
                }
            ),
            ptrs: Vec::new(),
            cache: None,
            thread_safe: true,
            dependency_gate: AtomicU32::new(0),
            // Associated nodes and trigger nodes will have the same dependency count
            dependency_count: last_rank.len() as u32,
        });
        // should make a register function for this
        current_rank.push(key);
        if let Some(current_trigger_node) = current_trigger_node {
            schedule.dep_graph.edges.get_mut(&current_trigger_node).unwrap().associates.push(key);
        } else {
            if schedule.dep_graph.root.is_none() {
                schedule.dep_graph.root = Some(key);
            }
            for i in last_rank.iter() {
                schedule.dep_graph.edges.get_mut(i).unwrap().dependents.push(key);
            }
            *current_trigger_node = Some(key)
        }
        schedule.dep_graph.edges.insert(key, NodeConnections::new(vec![], vec![]));
        (schedule, current_rank)
    }
}

pub struct Empty;
impl Destructure<Empty> for () {
    fn destructure<'a>(self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
        (schedule, current_rank)
    }
}

impl<A, B: Destructure<A>> SystemMethods<A> for B {
    fn order(self) -> Sequence<Self> {
        Sequence::<Self>(self)
    }
}


impl Destructure<SystemEntry> for Schedule {
    fn destructure<'a>(self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
        // Merging two DAGs
        // Get new key for each old key
        todo!();
        // let mut transition_map = FxHashMap::default();
        // let mut current = Vec::new();
        // let mut old = Vec::new();
        // let mut roots = FxHashSet::default();
        // for i in last_rank.iter() {
        //     for i in schedule.dep_graph.get(i).unwrap().dependents.iter() {
        //         roots.insert(*i);
        //     }
        // }
        // for (key, entry) in self.slot_map {
        //     let new_key = schedule.slot_map.insert(entry);
        //     transition_map.insert(key, new_key);
        //     current.push(new_key);
        //     old.push(key);
        //     current_rank.push(new_key);
        // }
        // // Iterate over the transition map to update the old last_rank
        // for key in old {
        //     let mut nodes = self.dep_graph.remove(&key).unwrap().dependents;
        //     for i in nodes.iter_mut() {
        //         *i = *transition_map.get(&i).unwrap()
        //     }
        //     let new_key = *transition_map.get(&key).unwrap();
        //     // only make the allow the node to point to the next stage if it is a root node
        //
        //     let mut eligible = vec![];
        //     for i in last_rank.iter() {
        //         let x = schedule.slot_map.get_mut(*i).unwrap();
        //         if x.dependency_count == 0 || !roots.contains(i) {
        //             if nodes.is_empty() {
        //                 eligible.push(*i);
        //                 x.dependency_count += 1;
        //             }
        //         }
        //     }
        //     nodes.extend(eligible);
        //     schedule.dep_graph.insert(new_key, NodeConnections::new(nodes, vec![]));
        // }
        // (schedule, current_rank)
    }
}




pub enum OwnershipType {
    Ref,
    Mut,
}

pub trait InnerAccess {
    const OWNERSHIP: (OwnershipType, TypeId);
    /// # Safety
    /// The pointer must point at a live value of the accessed resource type and, for mutable
    /// access, must not be aliased by any other reference for the lifetime of the result.
    unsafe fn fetch_data(pointer: NonNull<u8>) -> Self;
}


// Runtime function
impl<T: Any + Unwrap + 'static > InnerAccess for &T {
    const OWNERSHIP: (OwnershipType, TypeId) = (OwnershipType::Ref, TypeId::of::<T::INNER>());
    // Called alot of times, best to inline this
    #[inline(always)]
    unsafe fn fetch_data(pointer: NonNull<u8>) -> Self {
        unsafe { return &*(pointer.as_ptr() as *const T) }
    }
}

// Runtime function
impl<T: Any + Unwrap + 'static> InnerAccess for &mut T {
    const OWNERSHIP: (OwnershipType, TypeId) = (OwnershipType::Mut, TypeId::of::<T::INNER>());
    // Same here
    #[inline(always)]
    unsafe fn fetch_data(pointer: NonNull<u8>) -> Self {
        unsafe { return &mut *(pointer.as_ptr() as *mut T) }
    }
}




