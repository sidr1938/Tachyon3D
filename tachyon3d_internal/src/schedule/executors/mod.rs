use std::ptr::NonNull;
use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
use crate::ResourceHandler;
use crate::schedule::executors::single::SingleThreadedExecutor;
use crate::schedule::Schedule;
pub mod multi;
pub mod single;
pub trait ExecutorMethods {
    fn execute(&mut self, schedule: &mut Schedule, resources: &mut ResourceHandler);
}

pub enum Executor {
    SingleThreaded(SingleThreadedExecutor),
    MultiThreaded(MultiThreadedExecutor),
    Custom(Box<dyn ExecutorMethods>)
}

impl Default for Executor {
    fn default() -> Self {
        Executor::SingleThreaded(SingleThreadedExecutor)
    }
}

unsafe impl<T> Send for SendPointer<T> {}
unsafe impl<T> Sync for SendPointer<T> {}

impl<T> Clone for SendPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SendPointer<T> {}
#[repr(transparent)]
pub(crate) struct SendPointer<T> {
    inner: NonNull<T>
}
impl<T> SendPointer<T> {
    fn new_mut(ptr: &mut T) -> Self {
        SendPointer {
            inner: NonNull::from(ptr),
        }
    }
    fn new(ptr: &T) -> Self {
        SendPointer {
            inner: NonNull::from(ptr),
        }
    }

    /// # Safety
    /// The pointee must outlive every use of the returned reference and must not be aliased by
    /// any other reference while it is alive.
    pub unsafe fn as_mut(&self) -> &'static mut T {
        unsafe { &mut *self.inner.as_ptr() }
    }
    /// # Safety
    /// The pointee must outlive every use of the returned reference and must not be mutably
    /// aliased while it is alive.
    pub unsafe fn as_ref(&self) -> &'static T {
        unsafe { &*self.inner.as_ptr() }
    }
}
