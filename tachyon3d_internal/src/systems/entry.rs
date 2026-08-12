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