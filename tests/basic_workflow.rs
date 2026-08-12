use tachyon3d::*;


mod tests {
    use std::io;
    use std::io::Write;
    use slotmap::{new_key_type};
    use tachyon3d_internal::app::{AppT3D, Plugin};
    use tachyon3d_internal::schedule::executors::Executor;
    use tachyon3d_internal::schedule::executors::Executor::SingleThreaded;
    use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
    use tachyon3d_internal::systems::variadics::SystemMethods;
    use tachyon3d_internal::workgroups::{Extensions, Strategy, WorkgroupHandler};
    use tachyon3d_macros::Resource;

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

        let mut app = AppT3D::new();

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
        struct BasicLoop;
        struct ComputeTaskPool;
        struct IoTaskPool;
        struct SmallPool;
        let usable_threads = std::thread::available_parallelism().unwrap().get() - 1;
        app
            // External plugins eg: Third party fps controller, first party stuff
            .install(WorkgroupHandler::new())
            // Internal plugins, 2nd party, converting to an installation just involves changing the trait
            .add_plugins(Thing { tasks: vec![1; 100], })
            // Note: The executor implementation depends on the workgroup handler installation
            .add_schedule(BasicLoop, Executor::MultiThreaded(
                // Let the compute task pool have 50% of the threads we get,
                // IO gets 25%, and small pool gets 25%
                MultiThreadedExecutor::new(ComputeTaskPool)
            ))
            .add_workgroup(ComputeTaskPool, Strategy::WorkSteal, 7)
            // 0 threads still works it just doesnt spawn any threads or do anything, can be good as a load tester
            // for other workgroups
            .add_workgroup(IoTaskPool, Strategy::WorkSteal, 0)
            //.add_workgroup(SmallPool, Strategy::WorkSteal, 2)
            .add_resource(Dummy {
                value: 4,
            })
            .add_resource(Gummy {
            value: 2,
            })
            .add_resource(Blah {
            value: 2,
            })
            .add_systems(BasicLoop,
                         (((get_c,get_res,get_res,get_res), (get_res), (get_f, get_res, get_res, get_res, get_f, get_f)),
                         ((get_c,get_res,get_res,get_res).order(), (get_res), (get_f, get_res, get_res, get_res, get_f, get_f)),
                         ((get_c,get_res,get_res,get_res), (get_res), (get_f, get_res, get_res, get_res, get_f, get_f).order()),
                         ((get_c,get_res,get_res,get_res), (get_res), (get_f, get_res, get_res, get_res, get_f, get_f))).order()
            );


        let dot = app.schedules.get_mut(BasicLoop).unwrap().edges_to_dot();
        std::fs::write("graph.dot", dot).unwrap();

        app.schedules.get_mut(BasicLoop).unwrap().cache_pointers(&mut app.resources);
        for _ in 0..3 {
            app.schedules.get_mut(BasicLoop).unwrap().run(&mut app.resources);
            //std::thread::sleep(std::time::Duration::from_millis(100));
            app.full_sync();
        }
        app.full_shutdown(true);

        fn get_res(k: &Gummy, m: &mut Dummy) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            //panic![];
            println!("res");
            // for i in 0..1 {
            //     println!("a");
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
            //io::stdout().flush().unwrap();
        }

        fn get_k(k: &mut Gummy, m: &mut Dummy) {
            println!("k");
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
            //io::stdout().flush().unwrap();
        }

        fn get_m(k: &mut Gummy, m: &mut Dummy) {
            println!("m");
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
            //io::stdout().flush().unwrap();
        }
        fn get_f(k: &mut Gummy, m: &mut Dummy) {
            println!("f");
            for i in 0..1 {
                // let mut v = k.value;
                // let c = k.value;
                // std::println!("D {} {}", m.value, 2);
                // m.value += 1;
            }
            //io::stdout().flush().unwrap();
        }

        fn get_c(k: &mut Gummy, m: &mut Dummy) {
            println!("c");
            //std::println!("D {} {}", k.value, m.value);
            // for i in 0..1 {
            //     let mut v = k.value;
            //     let c = k.value;
            //     std::println!("D {} {}", m.value, 2);
            //     m.value += 1;
            // }
            //io::stdout().flush().unwrap();
        }

    }
}