pub mod destructure;
pub mod executors;

use crate::tachyon3d_internal::*;

macro_rules! parse_tuple_current_rank {
    ($($y:ident),*) => {
        // Destructure recursively
        impl<Fn, $($y: Any + InnerAccess),*> Destructure<($($y),*)> for Fn where Fn: FnMut($($y,)*) + 'static {
            // Conversion
            fn destructure<'a>(mut self, last_rank: &mut Vec<DefaultKey>, current_rank: &'a mut Vec<DefaultKey>, current_trigger_node: &'a mut Option<DefaultKey>, schedule: &'a mut Schedule) -> (&'a mut Schedule, &'a mut Vec<DefaultKey>) {
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
                        system: Box::new(move | mut pointers: ArgPointers | {
                            let mut arg = pointers.iter().copied();
                            self(
                                $(<$y as InnerAccess>::fetch_data(arg.next().expect("Missing Arguement"))),*
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

ptrs_type_erasure!(
    // Starter
    A,
    // Generated
    A,B,C,D,E,F,G,H,I,J,K,L,M,N,O,P,Q,R
);