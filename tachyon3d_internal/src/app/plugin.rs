use std::any::TypeId;
use crate::app::AppT3D;



pub trait RunAsOwned {
    fn runtime(self, app: AppT3D);
}
pub trait Plugin where Self: 'static {
    fn build(self, app: &mut AppT3D) where Self: Sized {
        app.resources.internal.insert(TypeId::of::<Self>(), Box::new(self));
    }
}

// Installations have more options with how they can be built,
// however other than that they are the exact same, but are the preferred method for external
// modifications, plugins can be converted to installations very easily by changing the trait impl
pub trait Installation where Self: 'static {
    fn build(self, app: &mut AppT3D) -> Self where Self: Sized {
        self
    }
    fn build_ref(&mut self, app: &mut AppT3D) where Self: Sized {}
    fn install_plugin(self, app: &mut AppT3D) where Self: Sized {
        let mut installation = self.build(app);
        installation.build_ref(app);
        app.resources.internal.insert(TypeId::of::<Self>(), Box::new(installation));
    }
}

