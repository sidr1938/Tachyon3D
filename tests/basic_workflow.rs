use tachyon3d::*;


mod tests {
    use crate::{DependencyGraph, ResourceHandler};
    use crate::{ Resource };
    use std::sync::atomic::{AtomicU32, Ordering};
    use slotmap::{new_key_type};
    use tachyon3d_internal::{DisjointedAccess};
    use tachyon3d_internal::systems::destructure::SystemMethods;
    use crate::{AppT3D, Plugin};
    //use crate::hyper_fiber::ParallelScheduler;

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

                }
            }
        }

        new_key_type! {
            pub struct SystemNodeKey;
        }

        fn jump() {
            println!("Jump");
        }

        let mut app = AppT3D::new();
        struct UpdateScheduler {
            pub systems: DependencyGraph,
        }

        use crate::InnerAccess;
        #[derive(Resource)]
        struct Dummy {
            value: u32
        }

        #[derive(Resource)]
        struct Gummy {
            value: u32
        }

        #[derive(Resource)]
        struct Blah {
            value: u32
        }

        struct Test<T> {
            thing: T
        }
        // add a warning feature
        // add event queue
        struct MTS;
        struct Quickstart;
        app
            .add_plugin(Thing { tasks: vec![1; 100], })
            .add_scheduler(Quickstart, Default::default())
            .insert_res(Dummy {
                value: 4,
            }).insert_res(Gummy {
            value: 2,
            }).insert_res(Blah {
            value: 2,
            }).add_systems(Quickstart,
                (
                    (get_c, get_res), (get_f, get_f), (get_m, get_res).order(),
                    ((get_c, get_c, (get_c, get_f, get_c)), get_c, get_res).order(),
                ).order()
            ).add_systems(Quickstart, (get_c,));
        // let dot = app.schedulers.get_mut(Quickstart).unwrap().edges_to_dot();
        // std::fs::write("graph.dot", dot).unwrap();

        app.schedulers.get_mut(Quickstart).unwrap().fetch_pointers(&mut app.resources);
        for _ in 0..5 {
            app.schedulers.get_mut(Quickstart).unwrap().run(&mut app.resources);
        }


        fn get_res(k: &Gummy, m: &mut Dummy) {
            //panic![];
            println!("res");
            // for i in 0..1 {
            //     println!("a");
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
        }

        fn get_k(k: &mut Gummy, m: &mut Dummy) {
            println!("k");
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
        }

        fn get_m(k: &mut Gummy, m: &mut Dummy) {
            println!("m");
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
        }
        fn get_f(k: &mut Gummy, m: &mut Dummy) {
            println!("f");
            for i in 0..1 {
                // let mut v = k.value;
                // let c = k.value;
                // std::println!("D {} {}", m.value, 2);
                // m.value += 1;
            }
        }

        fn get_c(k: &mut Gummy, m: &mut Dummy) {
            println!("c");
            std::println!("D {} {}", k.value, m.value);
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
        }

    }
}