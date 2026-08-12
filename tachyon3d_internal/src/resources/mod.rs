use std::any::{Any, TypeId};
use fxhash::FxHashMap;
pub mod access;
pub mod fetch;

pub struct ResourceHandler {
    pub internal: FxHashMap<TypeId, Box<dyn Any>>,
}

impl ResourceHandler {
    pub fn new() -> Self {
        Self {
            internal: Default::default(),
        }
    }
    pub fn insert<T: Any>(&mut self, resource: T) -> &mut Self {
        self.internal.insert(resource.type_id(), Box::new(resource));
        self
    }
    pub fn get<T: Any>(&mut self) -> Option<&T> {
        self.get_direct(&TypeId::of::<T>())
            .and_then(|r| r.downcast_ref())
    }
    pub fn get_direct(&mut self, key: &TypeId) -> Option<&Box<dyn Any>> {
        self.internal.get(key)
    }
    pub fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.get_mut_direct(&TypeId::of::<T>())
            .and_then(|r| r.downcast_mut())
    }
    pub fn get_mut_direct(&mut self, key: &TypeId) -> Option<&mut Box<dyn Any>> {
        self.internal.get_mut(key)
    }
    pub fn contains_key(&mut self, key: &TypeId) -> bool {
        self.internal.contains_key(&key)
    }
    // A failed downcast puts the resource back instead of dropping it on the floor
    pub fn remove<T: Any>(&mut self) -> Option<T> {
        let key = TypeId::of::<T>();
        match self.remove_direct(&key)?.downcast::<T>() {
            Ok(resource) => Some(*resource),
            Err(resource) => {
                self.internal.insert(key, resource);
                None
            }
        }
    }
    pub fn remove_direct(&mut self, key: &TypeId) -> Option<Box<dyn Any>> {
        self.internal.remove(key)
    }
}


