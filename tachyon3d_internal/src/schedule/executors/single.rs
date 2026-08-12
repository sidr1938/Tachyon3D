use std::ptr::addr_eq;
use crate::{ResourceHandler};
use crate::schedule::Schedule;

pub struct SingleThreadedExecutor;
impl SingleThreadedExecutor {
    pub fn execute(&mut self, schedule: &mut Schedule, resources: &mut ResourceHandler) {
        // To run our scheduler we need an executor,
        // typically you would want a multithreaded executor, but
        // for demonstration purposes we will go for a single thread
        // An executor can be made in anyway
        // but a basic one would start off with a queue
        let mut queue = vec![];
        // The dependency graph builder provides you with a slotmap,
        // directed acyclic graph, and a single root to start from
        // The algorithm batches parallel blocks, with a trigger node having associated nodes
        // to reduce memory costs of parallel - parallel block connections significantly
        // Sometimes the user wont build a plan right away and they are just prototyping, so if theres no root
        // there no plan, and we dont do anything
        if let Some(root) = &schedule.dep_graph.root {
            // Go through its assoicate nodes, reversing is optional, just makes unordered execution
            // happen forward in a single threaded basic executor if you want
            for i in schedule.dep_graph.edges.get(&root).unwrap().associates.iter().rev() {
                queue.push(i)
            }
            // Push the root up now
            queue.push(root);
        }
        // Keep looping while the queue has items
        while let Some(key) = queue.pop() {
            // Get the system entry (a system with metadata) in the slotmap via the key
            let sys = schedule.systems.get_mut(*key).unwrap();
            // A system will typically have arguements, get_disjoint_unchecked is a custom method
            // that handles it for us to get the data we need to input into the system
            // Just debug testing, not needed, everything is nominal if all schedule run orderly.
            // If you did what I did here using .rev() and putting the root after since queue is a .pop()
            // function (you can see the debug message go sequentially from 1v1 -> 2v1 -> 3v1...)
            // There is a bug if it is jumping around in a single threaded scheduler,
            // multithreaded is not gaurenteed, jumping around is expected there
            // if sys.label.is_some() {
            //     dbg![&sys.label];
            // } else {
            //     dbg![&key];
            // }
            // Deref it mutably so it can modify the state of our application
            // We already built a cache, however if we want to add schedule and dont want to build a cache
            // for those we need to add this branch to check for those that dont have a cache installed yet
            // The entry runs off of its cache when there is one, a cache builder pre-runs the
            // argument fetching so most runs avoid it entirely
            // the cache works well if that resource isnt deleted or replaced, which is like 80% of cases
            // if not, you have to rebuild the entire cache, later on ill improve this to only rebuild whats needed
            // that way caches only have to be rebuilt for schedule that have a resource that changed
            sys.run(resources);
            // Will review over this being a vector of default keys since this section is sort of
            // connected to the old algorithm, but technically there should only be one dependent pointer
            // at all times due to the new batching system
            // ! Check out the dot format of your schedule to see what I mean if you're confused !
            let next_node = schedule.dep_graph.edges.get(key).unwrap().dependents.iter();
            for i in next_node {
                // The dependent (Eg: the next trigger node with its associates), will have
                // a max gate that if the atomic count equals it that means all dependencies have ran.
                // The other way would be where the dependent checks if the dependencies are finished,
                // that would require polling and is much slower and unreliable
                // Once the last node finishes (can be the root aswell) within the parallel block
                // the gate is unlocked and the node pushes the next task upto the queue
                if schedule.systems.get(*i).unwrap().dependency_arrived() {
                    // Push all of its associates, if any
                    for k in schedule.dep_graph.edges.get(i).unwrap().associates.iter().rev() {
                        queue.push(k);
                    }
                    queue.push(i);
                }
            }
        }
    }
}


