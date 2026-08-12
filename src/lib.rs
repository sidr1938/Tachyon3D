extern crate self as tachyon3d;
pub use tachyon3d_internal::*;
pub use tachyon3d_macros::Resource;

#[cfg(feature = "dynamic_linking")]
#[allow(unused_imports)]
use tachyon3d_dylib as _;