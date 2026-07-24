// Tachyon3D API //
use std::any::{Any, TypeId};
use std::ops::{Deref, DerefMut};
use fxhash::FxHashMap;

// 0no Notes //

// Try to maintain > 2 dependencies
// Double indirection with receiving systems fixed
// Fixed a bug where removing a resource would remove the cache even if the resource wasnt in the catch

// An improvement (i forgot)
// Added tuples and tuple nesting instead of vecs
// Added turbofish to add_system instead of passing it as a argument, avoids runtime costs 
// (even if it is not a marker struct)



// qty Types //

pub type FnSystem = Box<dyn FnMut()>;

// qtr Traits //

pub trait Destructure {
    fn destructure(self) -> Vec<FnSystem>;

}

pub trait RunAsOwned {
    fn runtime(self, app: AppT3D);
}

pub trait Scheduler {
    fn systems(&mut self) -> &mut Vec<FnSystem>;
    fn add_system(&mut self, system: FnSystem);
}

pub trait Plugin where Self: 'static {
    fn build(self, app: &mut AppT3D) where Self: Sized {
        app.resources.insert(self);
    }
}

// qma Macros //
macro_rules! destructure {
    (($k:ident, $p:tt), ($y:ident, $n:tt)) => {
        // Edge cases eg: end of destructuring
        impl<F> Destructure for F where F: FnMut() + 'static {
            fn destructure(self) -> Vec<FnSystem> { vec![Box::new(self)] }
        }
        impl<F> Destructure for (F,) where F: FnMut() + 'static {
            fn destructure(self) -> Vec<FnSystem> { vec![Box::new(self.0)] }
        }
        impl Destructure for () { fn destructure(self) -> Vec<FnSystem> { vec![] } }
    };
    (($k:ident, $p:tt), $(($y:ident, $n:tt)),*) => {
        // Destructure rescursively
        impl<$($y: Destructure),*> Destructure for ($($y,)*) where $($y: Destructure),* {
            fn destructure(self) -> Vec<FnSystem> {
                let mut vec = vec![];
                $(vec.extend(self.$n.destructure());)*
                vec
            }
        }
        destructure!($(($y, $n)),*);
    };
}
// Pyramid of tuples ranging up to 16 max in one group, this macro allows for grouping tuples
// To bypass the limit
destructure!((R, 17),(Q, 16),(P, 15),(O, 14),(N, 13),(M, 12),(L, 11),(K, 10),(J, 9),
    (I, 8),(H, 7),(G, 6),(F, 5),(E, 4),(D, 3),(C, 2),(B, 1),(A, 0));


// qst 'Struct / Impl' Pairs //

pub struct SchedulerInUse {
    label: TypeId,
    pub scheduler: Box<dyn Scheduler>,
}

pub struct SchedulerHandler {
    pub cache: Option<SchedulerInUse>,
    pub internal: FxHashMap<TypeId, Box<dyn Scheduler>>,
}

impl SchedulerHandler {
    pub fn get_direct(&self, label: TypeId) -> Option<&dyn Scheduler> {
        if let Some(scheduler) = &self.cache {
            if scheduler.label == label {
                return Some(&*scheduler.scheduler);
            }
        }
        self.internal.get(&label).and_then(|f| Some(f.deref()))
    }
    pub fn get_mut_direct(&mut self, label: TypeId) -> Option<&mut (dyn Scheduler + 'static)> {
        if let Some(scheduler) = &mut self.cache {
            if scheduler.label == label {
                return Some(&mut *scheduler.scheduler);
            }
        }
        self.internal.get_mut(&label).and_then(move |f| Some(f.deref_mut()))
    }
    pub fn get<T: 'static>(&self) -> Option<&dyn Scheduler> {
        self.get_direct(TypeId::of::<T>())
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut (dyn Scheduler + 'static)> {
        self.get_mut_direct(TypeId::of::<T>())
    }
}
pub struct AppT3D {
    pub schedulers: SchedulerHandler,
    pub resources: ResourceHandler
}

impl AppT3D {
    pub fn new() -> Self {
        Self {
            schedulers: SchedulerHandler {
                cache: None,
                internal: FxHashMap::default(),
            },
            resources: ResourceHandler {
                cache: None,
                internal: FxHashMap::default()
            },
        }
    }

    // Could make a trait for this
    pub fn cache_scheduler<T: 'static>(&mut self) -> &mut Self {
        if let Some(old_scheduler) = self.schedulers.cache.take() {
            self.schedulers.internal.insert(old_scheduler.label, old_scheduler.scheduler);
        }
        self.schedulers.cache = Some(SchedulerInUse {
            label: TypeId::of::<T>(),
            scheduler: self.schedulers.internal.remove(&TypeId::of::<T>()).expect("No schedular exists")
        });
        self
    }
    pub fn cache_resource<T: 'static>(&mut self) -> &mut Self {
        if let Some(old_resource) = self.resources.cache.take() {
            self.resources.internal.insert(old_resource.label, old_resource.resource);
        }
        self.resources.cache = Some(ResourceInUse {
            label: TypeId::of::<T>(),
            resource: self.resources.internal.remove(&TypeId::of::<T>()).expect("No resource exists"),
        });
        self
    }



    pub fn add_systems<T: 'static>(&mut self, systems: impl Destructure) -> &mut Self {
        // Cached
        if let Some(active_scheduler) = &mut self.schedulers.cache {
            if active_scheduler.label == TypeId::of::<T>() {
                for system in systems.destructure() {
                    active_scheduler.scheduler.add_system(system);
                }
                return self;
            }
        }
        let scheduler = self.schedulers.internal.get_mut(&TypeId::of::<T>()).expect("No schedular exists");
        for system in systems.destructure() {
            scheduler.add_system(system);
        }
        self
    }
    pub fn add_scheduler<T: 'static>(&mut self, scheduler: Box<dyn Scheduler>) -> &mut Self {
        self.schedulers.internal.insert(TypeId::of::<T>(), scheduler);
        self
    }
    // Build on top of code
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        plugin.build(self);
        self
    }
    pub fn run_owned<R: RunAsOwned + 'static>(mut self) {
        self.resources.remove::<R>().unwrap().runtime(self);
    }

}

// Resource Handler Stuff
pub struct ResourceInUse {
    label: TypeId,
    pub resource: Box<dyn Any>,
}
pub struct ResourceHandler {
    pub cache: Option<ResourceInUse>,
    pub internal: FxHashMap<TypeId, Box<dyn Any>>,
}
impl ResourceHandler {
    pub fn new() -> Self {
        Self {
            cache: None,
            internal: Default::default(),
        }
    }
    pub fn insert<T: Any>(&mut self, resource: T) {
        self.internal.insert(resource.type_id(), Box::new(resource));
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        if let Some(cached_resource) = &self.cache {
            if cached_resource.label == TypeId::of::<T>() {
                return cached_resource.resource.downcast_ref();
            }
        }
        self.internal.get(&TypeId::of::<T>()).and_then(|r| r.downcast_ref())
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if let Some(cached_resource) = &mut self.cache {
            if cached_resource.label == TypeId::of::<T>() {
                return cached_resource.resource.downcast_mut();
            }
        }
        self.internal.get_mut(&TypeId::of::<T>())
            .and_then(|r| r.downcast_mut())
    }
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        if self.cache.is_some() {
            if self.cache.as_ref().unwrap().label == TypeId::of::<T>() {
                return self.cache.take().unwrap().resource.downcast().ok().map(|r| *r);
            }
        }
        self.internal.remove(&TypeId::of::<T>()).and_then(|f| f.downcast::<T>()
            .ok().map(|r| *r))
    }
}


// qte Tests //
mod tests {
    use crate::{AppT3D, Plugin, Scheduler, FnSystem};

    #[test]
    fn test() {

        pub struct Bad;
        pub struct Thing {
            tasks: Vec<u32>,
        }

        impl Plugin for Thing {}

        pub trait Extensions {
            fn doing<T>(&mut self) -> &mut Self
            where
                Self: Sized,
                T: RunAll + 'static
            ;
        };
        impl Extensions for AppT3D {
            fn doing<T>(&mut self) -> &mut Self
            where
                Self: Sized,
                T: RunAll + 'static
            {
                self.resources.get_mut::<T>().unwrap().run_all();

                self
            }
        }

        pub trait RunAll {
            fn run_all(&mut self);
        }
        impl RunAll for Thing {
            fn run_all(&mut self) {
                for i in 0..self.tasks.len() {
                    dbg!(i);
                }
            }
        }
        impl Thing {

        }

        fn test() {
            println!("Hello, world!");
        }
        fn jump() {
            println!("Jump");
        }
        let mut app = AppT3D::new();
        struct UpdateScheduler {
            systems: Vec<FnSystem>,
        }

        impl Scheduler for UpdateScheduler {
            fn systems(&mut self) -> &mut Vec<FnSystem> {
                &mut self.systems
            }
            fn add_system(&mut self, system: FnSystem) {
                self.systems.push(system)
            }
        }


        app.add_plugin(Thing { tasks: vec![1; 100], })
            .add_scheduler::<UpdateScheduler>(Box::new(UpdateScheduler { systems: vec![] }));
        app.add_systems::<UpdateScheduler>((test,(test, test)));
        app.doing::<Thing>().doing::<Thing>();
    }
}