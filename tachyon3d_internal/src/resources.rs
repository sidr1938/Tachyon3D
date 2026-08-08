use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::ptr::NonNull;
use fxhash::FxHashMap;
use crate::{OwnershipType};

pub struct ResourceHandler {
    pub internal: FxHashMap<TypeId, Box<dyn Any>>,
    pub ffi_internal: FxHashMap<TypeId, Box<dyn Any>>,
}

impl ResourceHandler {
    pub fn new() -> Self {
        Self {
            internal: Default::default(),
            ffi_internal: Default::default(),
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
    pub fn remove<T: Any>(&mut self) -> Option<T> {
        self.remove_direct(&TypeId::of::<T>())
            .and_then(|f| f.downcast::<T>()
                .ok().map(|r| *r))
    }
    pub fn remove_direct(&mut self, key: &TypeId) -> Option<Box<dyn Any>> {
        self.internal.remove(key)
    }
}

pub trait DisjointedAccess {
    fn get_disjoint(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Option<Vec<NonNull<u8>>>;
    fn get_disjoint_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<NonNull<u8>>;
}

impl DisjointedAccess for FxHashMap<TypeId, Box<dyn Any>> {
    // Safety checks to validate inputs with some runtime overhead
    fn get_disjoint(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Option<Vec<NonNull<u8>>> {
        let mut data = HashSet::new();
        // For each one, check for duplicates, or any invalid resources
        // This checks locally, as in within the function, not across functions
        // Note for future optimization,
        // this check can be done while importing the function to avoid runtime safety checks
        // next time extend rather than do seperate loops
        for (_,key) in key_pointers {
            if data.insert(key) || !self.contains_key(key) {
                return None
            }
        }
        Some(self.get_disjoint_unchecked(key_pointers))
    }
    // Good to use after you've confirmed your systems are valid
    fn get_disjoint_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<NonNull<u8>> {
        let mut data = Vec::new();
        let mut thread_safe = true;
        // Remove owned values beforehand because the memory location is affected
        for (ownership, key) in key_pointers {
            match ownership {
                OwnershipType::Mut => unsafe {
                    let resource = NonNull::new_unchecked(self.get_mut(key).unwrap().as_mut() as *mut dyn Any as *mut u8);
                    data.push(resource)
                }
                OwnershipType::Ref => unsafe {
                    let resource = NonNull::new_unchecked(self.get(key).unwrap().as_ref() as *const dyn Any as *mut u8);
                    data.push(resource)
                }
            }
        }
        data
    }
}
