use std::alloc::System;
use std::any::TypeId;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use crate::ResourceHandler;
use crate::resources::access::NonSend;
use crate::resources::fetch::DisjointedAccess;
use crate::systems::variadics::{OwnershipType};
unsafe impl Send for SystemEntry {}
unsafe impl Sync for SystemEntry {}

unsafe impl Send for SendSyncNonNull {}
unsafe impl Sync for SendSyncNonNull {}
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct SendSyncNonNull {
    pub non_null: NonNull<u8>
}
impl SendSyncNonNull {
    pub fn from(non_null: NonNull<u8>) -> Self {
        SendSyncNonNull { non_null }
    }
}
pub struct SystemEntry {
    pub system: Box<dyn FnMut(&[SendSyncNonNull]) + Send + Sync>,
    pub label: Option<String>,
    pub ptrs: Vec<(OwnershipType, TypeId)>,
    pub cache: Option<Vec<SendSyncNonNull>>,
    pub thread_safe: bool,
    pub dependency_gate: AtomicU32,
    pub dependency_count: u32,
}

impl SystemEntry {
    // An unlabelled, uncached entry, dependency_count is the size of the rank it depends on
    pub fn new(
        system: Box<dyn FnMut(&[SendSyncNonNull]) + Send + Sync>,
        ptrs: Vec<(OwnershipType, TypeId)>,
        dependency_count: u32,
    ) -> Self {
        SystemEntry {
            system,
            label: None,
            ptrs,
            cache: None,
            // Unused right now
            thread_safe: true,
            dependency_gate: AtomicU32::new(0),
            dependency_count,
        }
    }
    // Counts a finished dependency, returning true once the last one arrived and this entry can
    // be dispatched, the gate resets itself so it is ready for the next run
    pub fn dependency_arrived(&self) -> bool {
        let previous = self.dependency_gate.fetch_add(1, Ordering::Acquire);
        if previous + 1 == self.dependency_count {
            self.dependency_gate.store(0, Ordering::Release);
            return true;
        }
        false
    }
    // Runs the system with its cached argument pointers if the cache was built, otherwise the
    // pointers are fetched from the resources on the spot
    pub fn run(&mut self, resources: &mut ResourceHandler) {
        if let Some(cache) = self.cache.as_ref() {
            (self.system)(cache);
        } else {
            let data = resources.internal.fetch_args_unchecked(&self.ptrs);
            (self.system)(&data);
        }
    }
}