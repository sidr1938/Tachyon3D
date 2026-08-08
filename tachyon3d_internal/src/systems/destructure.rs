use std::sync::atomic::AtomicU32;
use fxhash::{FxHashMap, FxHashSet};
use slotmap::DefaultKey;
use tachyon3d_internal::Schedule;
use crate::{ArgPointers, ScheduleBuilder, NodeConnections, SystemEntry};

pub trait Destructure<T> {
    fn destructure<'a>(self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>);

}
pub struct Sequence<T>(T);
pub trait SystemMethods<T> {
    fn order(self) -> Sequence<Self> where Self: Sized;
}

impl<F> Destructure<()> for F where F: FnMut() + 'static {
    // Conversion
    fn destructure<'a>(mut self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
        let key = schedule.systems.insert(SystemEntry {
            label: None,
            system: Box::new(
                move |_: ArgPointers| {
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

impl<T> Destructure<T> for () {
    fn destructure<'a>(self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
        (schedule, current_rank)
    }
}

impl<A, B: Destructure<A>> SystemMethods<A> for B {
    fn order(self) -> Sequence<Self> {
        Sequence::<Self>(self)
    }
}

macro_rules! destructure {
    (($_0:ident, $_1:ident, $_2:tt)) => {
        // EoD
    };
    // Snip off the first element to drive recursion
    // Avoid reverse later on
    (($_0:ident, $_1:ident, $_2:tt), $(($y:ident, $ym:ident, $n:tt)),*) => {
        // Destructure recursively
        impl<$($ym,)* $($y: Destructure<$ym>,)*> Destructure<($($ym,)*)> for ($($y,)*) {
            fn destructure<'a>(self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
                // Turn into a tuple
                let ($($y,)*) = self;
                $($y.destructure(last_rank, current_rank, current_trigger_node, schedule);)*
                (schedule, current_rank)
            }
        }
        destructure!($(($y, $ym, $n)),*);
    };
}

macro_rules! destructure_seq {
    (($_0:ident, $_1:ident, $_2:tt)) => {};
    (($_0:ident, $_1:ident, $_2:tt), $(($y:ident, $ym:ident, $n:tt)),*) => {
        // Destructure recursively
        impl<$($ym,)* $($y: Destructure<$ym>,)*> Destructure<($($ym,)*)> for Sequence<($($y,)*)> {
            fn destructure<'a>(self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
                 let ($($y,)*) = self.0;
                 // ((get_m, get_res, get_f), (get_f, (get_res, get_res).order()).order(), (get_f)).order()
                 // (MARKER6, get_c, (((MARKER5, get_c), get_c, (MARKER4, get_f, MARKER3)).order(), MARKER2)),
                 // This avoids making other nodes outside of the .order() scope a dependency,
                 // eg: MARKER5 wont depend on get_c
                 //    (MARKER1, get_c, get_c),
                 //    (MARKER2, get_c, get_res)
                 let mut local_last_rank: Vec<DefaultKey> = last_rank.clone();
                 let mut local_current_rank: Vec<DefaultKey> = vec![];
                 // This preserves trigger nodes in the outerscope, eg so that MARKER2 still gets the trigger node
                 // that is MARKER6, and its not erased
                 let mut old_current_trigger_node = *current_trigger_node;
                 $(
                    $y.destructure(&mut local_last_rank, &mut local_current_rank, current_trigger_node, schedule);
                    if old_current_trigger_node.is_none() {
                      old_current_trigger_node = *current_trigger_node;
                    }
                    local_last_rank = std::mem::take(&mut local_current_rank);
                    // So MARKER6 trigger associates with MARKER5 but doesnt make get_c an associate, nor the later parts
                   *current_trigger_node = None;
                 )*
                 // The end of the sequence can connect with later sequences as if it was an associated node to the earlier parallel block
                 current_rank.extend(local_last_rank);
                 // Keeps the trigger node from earlier nodes
                 // Eg: (MARKER6, get_c, ((MARKER5, get_c, (MARKER4, get_f, MARKER3)).order(), MARKER2))
                 // MARKER 2 is not forgotten as a trigger node
                 *current_trigger_node = old_current_trigger_node;
                (schedule, current_rank)
            }
        }
        destructure_seq!($(($y, $ym, $n)),*);
    };
}

impl Destructure<SystemEntry> for Schedule {
    fn destructure<'a>(self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
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

destructure!(
    (X, XM, 0),
    (R, RM, 0),
    (Q, QM, 1),(P, PM, 2),(O, OM, 3),(N, NM, 4),(M, MM, 5),
    (L, LM, 6),(K, KM, 7),(J, JM, 8),(I, IM, 9),(H, HM, 10),(G, GM, 11),
    (F, FM, 12),(E, EM, 13),(D, DM, 14),(C, CM, 15),(B, BM, 16),(A, AM, 17)
);

destructure_seq!(
    (X, XM, 0),
    (R, RM, 0),
    (Q, QM, 1),(P, PM, 2),(O, OM, 3),(N, NM, 4),(M, MM, 5),
    (L, LM, 6),(K, KM, 7),(J, JM, 8),(I, IM, 9),(H, HM, 10),(G, GM, 11),
    (F, FM, 12),(E, EM, 13),(D, DM, 14),(C, CM, 15),(B, BM, 16),(A, AM, 17)
);