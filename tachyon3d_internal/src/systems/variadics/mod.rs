
pub mod macros;

use std::any::{Any, TypeId};
use std::ptr::NonNull;
use fxhash::{FxHashMap, FxHashSet};
use crate::resources::access::Unwrap;
use crate::schedule::{Schedule};
use crate::schedule::graph::SystemKey;
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
        let entry = SystemEntry::new(
            Box::new(
                move |_: &[SendSyncNonNull]| {
                    self();
                }
            ),
            Vec::new(),
            // Associated nodes and trigger nodes will have the same dependency count
            last_rank.len() as u32,
        );
        schedule.register_system(entry, last_rank, current_rank, current_trigger_node);
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




