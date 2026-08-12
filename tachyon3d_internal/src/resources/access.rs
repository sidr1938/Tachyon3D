use std::any::Any;
use std::ops::{Deref, DerefMut};
use crate::Resource;

pub trait Unwrap { type INNER: Any; }
impl<T: Any + Resource + Send + Sync> Unwrap for T { type INNER = T; }
pub trait Wrap<T> {
    fn wrap(data: T) -> Self;
}
impl<T: Any + Resource + Send + Sync> Wrap<T> for T {
    fn wrap(data: T) -> Self {
        data
    }
}

// Removes metadata, the struct will be the exact same size as the data it represents, making
// it a "transparent" wrapper allowing us to do safe type conversions via pointers
#[repr(transparent)]
pub struct NonSend<T: Any + 'static> {
    inner: T,
}
// For explicitly defined syntax, i considered a couple of options quite thoroughly
/*
1) mut foo: WrapperMut<Inner>, foo: Wrapper<Inner>, foo: WrapperOwned<Inner>
2) mut foo: Wrapper<&mut Inner>, foo: Wrapper<&Inner>, foo: Wrapper<Inner>
3) foo: &mut Wrapper<Inner>, foo: &Wrapper<Inner>, foo: Wrapper<Inner> (eventually)
*/

/*
Option 3 seemed the best in terms of cooperation with the already existing
syntax for resources which is just &Inner, &mut Inner (no wrapper needed)
and it looked idiomatic/nice enough while avoiding too much verbosity or wrappers
*/
impl<T> Deref for NonSend<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for NonSend<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

