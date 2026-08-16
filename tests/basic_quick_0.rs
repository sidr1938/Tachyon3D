use tachyon3d::*;

mod tests {
    use std::any::Any;
    use std::io;
    use std::io::Write;
    use tachyon3d_internal::app::AppT3D;
    use tachyon3d_internal::ecs::{ECSPlugin, ECS};
    use tachyon3d_internal::schedule::executors::Executor;
    use tachyon3d_internal::schedule::executors::Executor::{MultiThreaded, SingleThreaded};
    use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
    use tachyon3d_internal::schedule::executors::single::SingleThreadedExecutor;
    use tachyon3d_internal::systems::variadics::SystemMethods;
    use tachyon3d_internal::workgroups::{Extensions, Strategy, Workgroup, WorkgroupHandler, WorkgroupPlugin};
    struct ComputeTP;
    struct Update;
    fn hello_world() {
        //std::thread::sleep(std::time::Duration::from_millis(100));
        println!("Hello, world!");
    }
    #[test]
    fn test() {
        AppT3D::new()
            // INP
            .install(ECSPlugin)
            // USABLE - UNSTABLE - UNOPTIMIZED
            .install(WorkgroupPlugin)
            .add_workgroup(ComputeTP, Strategy::WorkSteal, 7)
            .add_schedule(Update, Executor::MultiThreaded(MultiThreadedExecutor::new(ComputeTP)))
            .add_systems(Update, hello_world)
            .run(Update)
            .full_shutdown();
    }
}

#[derive(Resource)]
pub struct ReadResource {
    pub multipliers: Vec<f32>,
}
#[derive(Resource)]
pub struct WriteResource {
    pub positions: Vec<f32>,
}
mod tachyon3d_load_test {
    use super::*;
    use tachyon3d::*;
    use tachyon3d_internal::app::AppT3D;
    use tachyon3d_internal::schedule::executors::Executor;
    use tachyon3d_internal::schedule::executors::multi::MultiThreadedExecutor;
    use tachyon3d_internal::schedule::executors::single::SingleThreadedExecutor;
    use tachyon3d_internal::systems::variadics::SystemMethods;
    use tachyon3d_internal::workgroups::{Extensions, Strategy, WorkgroupHandler};

    struct ComputeTP;
    struct Update;

    // Parallel reader systems (can run concurrently)
    fn system_read_1(read: &ReadResource) {
        let sum: f32 = read.multipliers.iter().map(|x| x.sin().cos()).sum();
        std::hint::black_box(sum);
    }

    fn system_read_2(read: &ReadResource) {
        let sum: f32 = read.multipliers.iter().map(|x| x.tan().abs()).sum();
        std::hint::black_box(sum);
    }

    // Writer systems (require exclusive access to WriteResource)
    fn system_write_1(read: &ReadResource, write: &mut WriteResource) {
        for (p, m) in write.positions.iter_mut().zip(read.multipliers.iter()) {
            *p += m * 0.016;
        }
    }

    fn system_write_2(write: &mut WriteResource) {
        for p in write.positions.iter_mut() {
            //println!("D");
            *p *= 1.0001;
        }
    }

    #[test]
    fn benchmark_tachyon3d() {
        let element_count = 1_000_000;
        let read_res = ReadResource { multipliers: vec![1.5; element_count] };
        let write_res = WriteResource { positions: vec![0.0; element_count] };



        let mut app = AppT3D::new();
        // Insert initial resources if Tachyon3D supports resource registration
        // app.insert_resource(read_res);
        // app.insert_resource(write_res);
        app.add_resource(ReadResource { multipliers: vec![1.5; element_count] });
        app.add_resource(WriteResource { positions: vec![0.0; element_count] });
        app.install(WorkgroupHandler::new())
            .add_workgroup(ComputeTP, Strategy::WorkSteal, 4)
            .add_schedule(Update, Executor::MultiThreaded(MultiThreadedExecutor::new(ComputeTP)))
            //.add_schedule(Update, Executor::SingleThreaded(SingleThreadedExecutor))
            .cache_inputs(Update)
            .add_systems(
                Update,
                (
                    system_read_1,
                    system_read_2,
                    (system_write_1,
                    system_write_2,).order()
                ),
            );
        let start = std::time::Instant::now();
        // Run schedule for 100 iterations under load
        for _ in 0..100 {
            app.run(Update)
                .full_sync();
        }


        app.full_shutdown();
        println!("Tachyon3D Elapsed time: {:?}", start.elapsed());
    }
}