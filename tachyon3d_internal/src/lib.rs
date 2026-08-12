// Tachyon3D API //

extern crate self as tachyon3d_internal;
extern crate core;

pub use crate::resources::ResourceHandler;
pub trait Resource {}
pub mod resources;


pub mod schedule;
pub mod app;
pub mod systems;
pub mod workgroups;
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
Added a dense slot map storage for schedule
Minor-Minor:
(INTER-UPDATE) Switched from DefaultKey to SystemKey for the slotmap that stores systems
(INTER-UPDATE) Switched from backwards to forward tuple destructuring
Patches:

Removals:
    Removed cache for resources, FxHashMap is pretty fast and honestly the micro optimization wasnt
    worth it since checks had to be done everytime for a cache for most methods
    Removed owned values
*/


// Things to add
/*
Async queues needed
Dynamic adding and removing of schedule at runtime
Dynamic adding and removing of plugins at runtime
 */

// Optimizations to implenent
/*
Finished all optimization ideas for now.
*/
