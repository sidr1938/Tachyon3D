use std::any::{Any, TypeId};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use std::thread::{JoinHandle, ScopedJoinHandle};
use rustc_hash::FxHashMap;
use tachyon3d_internal::app::plugin::Installation;
use crate::{Resource, ResourceHandler};
pub use crate::app::plugin::{Plugin, RunAsOwned};
use crate::schedule::executors::Executor;
use crate::schedule::{Schedule, ScheduleHandler};
use crate::systems::variadics::Destructure;

pub mod plugin;

pub struct AppT3D {
    // Stable
    pub schedules: ScheduleHandler,
    pub resources: ResourceHandler,
}



impl AppT3D {
    pub fn new() -> Self {
        Self {
            schedules: ScheduleHandler {
                internal: FxHashMap::default(),
            },
            resources: ResourceHandler {
                internal: FxHashMap::default(),
            },
        }
    }
    // Not really a full 'ecs' system and not sure if i want to go with an integrated ecs
    pub fn add_systems<T: 'static, U>(&mut self, schedule: T, systems: impl Destructure<U>) -> &mut Self {
        let scheduler = self.schedules.internal.get_mut(&schedule.type_id()).expect("No schedular exists");
        scheduler.add_systems(systems);
        self
    }
    pub fn add_schedule<T: 'static>(&mut self, label: T, executor: Executor) -> &mut Self {
        self.schedules.internal.insert(label.type_id(), Schedule::new(Some(executor)));
        self
    }
    // External functionality just like how bevy does it
    pub fn add_plugins<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        plugin.build(self);
        self
    }
    pub fn install<I: Installation>(&mut self, installation: I) -> &mut Self {
        installation.install_plugin(self);
        self
    }
    // Convenience method
    pub fn add_resource<T: Any + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.resources.internal.insert(TypeId::of::<T>(), Box::new(resource));
        self
    }
    // Kind of a convenience method
    pub fn run_owned<R: RunAsOwned + Any>(mut self) {
        self.resources.remove::<R>().unwrap().runtime(self);
    }
}



