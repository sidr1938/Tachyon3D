use crate::systems::entry::SendSyncNonNull;
use crate::systems::variadics::InnerAccess;
use crate::systems::variadics::Sequence;
use std::any::{Any, TypeId};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use fxhash::{FxHashMap, FxHashSet};
use super::SystemKey;
use crate::schedule::{Schedule};
use crate::schedule::graph::NodeConnections;
use crate::systems::entry::SystemEntry;
use super::Destructure;
macro_rules! destructure {
    (($_0:ident, $_1:ident, $_2:tt)) => {
        // EoD
    };
    // Snip off the first element to drive recursion
    // Avoid reverse later on
    (($_0:ident, $_1:ident, $_2:tt), $(($y:ident, $ym:ident, $n:tt)),*) => {
        // Destructure recursively
        impl<$($ym,)* $($y: Destructure<$ym>,)*> Destructure<($($ym,)*)> for ($($y,)*) {
            fn destructure<'a>(self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
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
            fn destructure<'a>(self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
                 let ($($y,)*) = self.0;
                 // ((get_m, get_res, get_f), (get_f, (get_res, get_res).order()).order(), (get_f)).order()
                 // (MARKER6, get_c, (((MARKER5, get_c), get_c, (MARKER4, get_f, MARKER3)).order(), MARKER2)),
                 // This avoids making other nodes outside of the .order() scope a dependency,
                 // eg: MARKER5 wont depend on get_c
                 //    (MARKER1, get_c, get_c),
                 //    (MARKER2, get_c, get_res)
                 let mut local_last_rank: Vec<SystemKey> = last_rank.clone();
                 let mut local_current_rank: Vec<SystemKey> = vec![];
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

macro_rules! parse_tuple_current_rank {
    ($($y:ident),*) => {
        // Destructure recursively
        impl<Fn, $($y: Any + InnerAccess),*> Destructure<($($y),*)> for Fn where Fn: FnMut($($y,)*) + 'static, Fn: std::marker::Send, Fn: std::marker::Sync {
            // Conversion
            fn destructure<'a>(mut self, last_rank: &mut Vec<SystemKey>, current_rank: &'a mut Vec<SystemKey>, current_trigger_node: &'a mut Option<SystemKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<SystemKey>) {
                let mut ptrs = vec![];
                // These basically send the type
                $(
                    // OwnershipType is only used to figure if the data is owned mut or const, not used outside this
                    ptrs.push(<$y as InnerAccess>::OWNERSHIP);
                )*
                let key = schedule.systems.insert(
                    SystemEntry {
                        // Get disjoint translates KeyPointers into Box<dyn Any> to pass into the res pointers hashmap
                        label: None,
                        system: Box::new(move | mut pointers: &[SendSyncNonNull] | {
                            let mut arg = pointers.iter().copied();
                            self(
                                $(<$y as InnerAccess>::fetch_data(arg.next().expect("Missing Arguement").non_null)),*
                            );
                        }),
                        ptrs,
                        cache: None,
                        // Unused right now
                        thread_safe: true,
                        dependency_gate: AtomicU32::new(0),
                        // Associated nodes and trigger nodes will have the same dependency count
                        dependency_count: last_rank.len() as u32,
                    }
                );
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
    };
}

macro_rules! ptrs_type_erasure {
    ($_0:tt, $y:ident) => {
        // Handle the 1 argument case (the tail / end of recursion)
        parse_tuple_current_rank!($y);
    };
    // Snip off the first element to drive recursion
    ($_0:tt, $($y:ident),*) => {
        parse_tuple_current_rank!($($y),*);
        ptrs_type_erasure!($($y),*);
    };
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

ptrs_type_erasure!(
    // Starter
    A,
    // Generated
    A,B,C,D,E,F,G,H,I,J,K,L,M,N,O,P,Q,R
);