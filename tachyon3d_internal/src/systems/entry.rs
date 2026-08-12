use std::alloc::System;
use std::any::TypeId;
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use crate::resources::access::NonSend;
use crate::systems::variadics::{OwnershipType};
unsafe impl Send for SystemEntry {}
unsafe impl Sync for SystemEntry {}

unsafe impl Send for SendSyncNonNull {}
unsafe impl Sync for SendSyncNonNull {}
// Send + Sync is asserted for this pointer, so it can only be built through an unsafe
// constructor, a safe one would let any pointer be shared across threads from safe code
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct SendSyncNonNull {
    non_null: NonNull<u8>
}
impl SendSyncNonNull {
    /// # Safety
    /// The pointee must be sound to access from any thread for as long as this wrapper is used,
    /// which includes staying alive and not being aliased by a conflicting reference.
    pub unsafe fn new(non_null: NonNull<u8>) -> Self {
        SendSyncNonNull { non_null }
    }
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.non_null
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