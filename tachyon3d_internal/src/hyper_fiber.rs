// use crate::{tachyon3d_internal, ArgPointers, DependencyGraphBuilder, ResourceHandler, SystemEntry};
// use crossbeam_deque;
// use crossbeam_deque::{Injector, Worker};
// use tachyon3d_internal::{Scheduler};
// // Tachyons work stealing and parallelizing library
// // The original goal was to add fibers, but im not sure ill do that unless i see a use for it
// // otherwise its unneeded overhead for now, im sticking with the name cause its tuff
//
// // Workers use lifo, helps allow thieves get old tasks that are likely chunkier so they have plenty to chew on
// // while using fifo on the global queue to its local queue
//
//
// pub struct ParallelScheduler {
//     tasks: Vec<SystemEntry>,
//     master_deque: Injector<SystemEntry>,
//     main_thread: Vec<Box<dyn FnMut(ArgPointers)>>,
//     thread_pool: usize,
// }
// impl Scheduler for ParallelScheduler {
//     fn digraph(&mut self) -> &mut DependencyGraphBuilder {
//         todo!()
//     }
//
//
//     fn run(&mut self, resources: &mut ResourceHandler) {
//         while let Some(task) = self.tasks.pop() {
//             self.master_deque.push(task);
//         }
//         //let resources = UnsafeCell::new(resources.internal);
//         std::thread::scope(|scope| {
//             let mut workers:  Vec<Worker<SystemEntry>> = vec![];
//             let mut stealers = vec![];
//             let mut handles = vec![];
//             for i in 0..self.thread_pool {
//                 let worker: Worker<SystemEntry> = Worker::new_lifo();
//                 stealers.push(worker.stealer());
//                 workers.push(worker);
//             }
//
//
//
//             let master_deque = &self.master_deque;
//             for (thread_id, worker ) in workers.into_iter().enumerate() {
//                 handles.push(scope.spawn(move || {
//
//                     // loop {
//                     //     break;
//                     //     dbg![thread_id];
//                     //     match worker.pop() {
//                     //         Some(mut s) => {
//                     //             let sys = &mut s.system;
//                     //             //let data = resources.get_disjoint_unchecked(&s.ptrs).unwrap();
//                     //             //sys(data.pointers);
//                     //             // Push the task back into the storage for another loop
//                     //             //self.tasks.push(s);
//                     //             continue;
//                     //         },
//                     //         None => {}
//                     //     }
//                     // //
//                     // //     match master_deque.steal_batch_with_limit_and_pop(&worker, 2) {
//                     // //         Steal::Empty => {
//                     // //             if self.thread_pool == 1 {
//                     // //                 break;
//                     // //             }
//                     // //         },
//                     // //         Steal::Success(mut s) => {
//                     // //             let sys = &mut s.system;
//                     // //             let data = resources.get_disjoint_unchecked(&s.ptrs).unwrap();
//                     // //             sys(data.pointers);
//                     // //             //self.tasks.push(s);
//                     // //             continue;
//                     // //         },
//                     // //         Steal::Retry => {
//                     // //             continue;
//                     // //         },
//                     // //     }
//                     // //
//                     // //     if self.thread_pool > 1 {
//                     // //         // Shutdown condition
//                     // //         let rand_index = rand::random_range(0..(self.thread_pool));
//                     // //         let mut offset = 0;
//                     // //         while !stealers[(rand_index + offset) % self.thread_pool].is_empty() {
//                     // //             offset += 1;
//                     // //             if offset > self.thread_pool {
//                     // //                 break;
//                     // //             }
//                     // //         }
//                     // //         if offset > self.thread_pool {
//                     // //             dbg![thread_id];
//                     // //             break;
//                     // //         }
//                     // //         // Found a victim to steal from
//                     // //         let victim = &stealers[rand_index + offset % self.thread_pool];
//                     // //         match victim.steal_batch_with_limit_and_pop(&worker, 2) {
//                     // //             Steal::Empty => {
//                     // //
//                     // //                 continue;
//                     // //             },
//                     // //             Steal::Success(mut s) => {
//                     // //                 let sys = &mut s.system;
//                     // //                 let data = resources.get_disjoint_unchecked(&s.ptrs).unwrap();
//                     // //                 sys(data.pointers);
//                     // //                 //self.tasks.push(s);
//                     // //                 continue;
//                     // //             },
//                     // //             Steal::Retry => {
//                     // //                 continue;
//                     // //             },
//                     // //         }
//                     // //     }
//                     // }
//
//                 }));
//             }
//         });
//     }
// }
//
// impl ParallelScheduler {
//     pub fn new(thread_pool: usize) -> ParallelScheduler {
//         ParallelScheduler {
//             tasks: Vec::new(),
//             thread_pool,
//             master_deque: Injector::new(),
//             main_thread: Vec::new(),
//         }
//     }
//
// }
// //
// // fn jump() {
// //     for i in 0..10 {
// //         std::thread::sleep(std::time::Duration::from_millis(2));
// //     }
// // }
// // fn jump2() {
// //     println!("LONG TASK");
// //     for i in 0..10 {
// //         std::thread::sleep(std::time::Duration::from_millis(20));
// //     }
// //
// // }
// // mod tests {
// //     use std::thread;
// //     use crossbeam_deque::{Steal, Worker};
// //     use crate::hyper_fiber::{jump, jump2, HyperFiber};
// //
// //     #[test]
// //     fn test() {
// //         let mut thing = HyperFiber::new(1);
// //         let mut tasks: Vec<Box<dyn FnMut() + Send + Sync>> = Vec::new();
// //         for i in 0..8 {
// //             thing.add_thingies(vec![Box::new(jump)]);
// //             tasks.push(Box::new(jump as fn()))
// //         }
// //         for i in 0..1 {
// //             thing.add_thingies(vec![Box::new(jump2)]);
// //             tasks.push(Box::new(jump2 as fn()))
// //         }
// //         for i in 0..16 {
// //             thing.add_thingies(vec![Box::new(jump)]);
// //             tasks.push(Box::new(jump as fn()))
// //         }
// //         if false {
// //             for mut i in tasks {
// //                 i.as_mut()();
// //             }
// //         } else {
// //             std::thread::scope(|thread| {
// //                 let worker = Worker::new_lifo();
// //                 let stealer = worker.stealer();
// //                 let worker2 = Worker::new_lifo();
// //                 let stealer2 = worker2.stealer();
// //
// //                 let global_deque_ref = &thing.scheduler.master_deque;
// //
// //                 let h1 = thread.spawn(move || {
// //                     let mut tasks_done = 0;
// //                     loop {
// //                         if worker.is_empty() {
// //                             match global_deque_ref.steal_batch_with_limit_and_pop(&worker, 32) {
// //
// //                                 Steal::Empty => {
// //                                     match stealer2.steal_batch_with_limit_and_pop(&worker, 2) {
// //                                         Steal::Empty => {
// //                                             dbg!["Thread 1 is done"];
// //                                             break;
// //                                         }
// //                                         Steal::Success(mut x) => {
// //                                             x.as_mut()();
// //                                         }
// //                                         Steal::Retry => {
// //                                             println!["LOCK COLLISION"];
// //                                             continue
// //                                         }
// //                                     }
// //                                 },
// //                                 Steal::Success(mut data) => {
// //                                     println!("THREAD 1: Tasks {}", tasks_done);
// //                                     tasks_done += 1;
// //                                     data.as_mut()();
// //                                 },
// //                                 Steal::Retry => {
// //                                     println!["LOCK COLLISION"];
// //                                     continue;
// //                                 },
// //                             }
// //                         } else {
// //                             println!("Executing Local tasks for thread 1");
// //                             let mut x = worker.pop().unwrap();
// //                             x.as_mut()();
// //                         }
// //                         thread::sleep(std::time::Duration::from_millis(2000));
// //                     }
// //                 });
// //                 let h2 = thread.spawn(move || {
// //                     let mut tasks_done = 0;
// //                     loop {
// //                         if worker2.is_empty() {
// //                             match global_deque_ref.steal_batch_with_limit_and_pop(&worker2, 2) {
// //
// //                                 Steal::Empty => {
// //
// //                                     match stealer.steal_batch_with_limit_and_pop(&worker2, 2) {
// //                                         Steal::Empty => {
// //                                             dbg!["Thread 2 is done"];
// //                                             break;
// //                                         }
// //                                         Steal::Success(mut x) => {
// //                                             x.as_mut()();
// //                                         }
// //                                         Steal::Retry => {
// //                                             println!["LOCK COLLISION"];
// //                                             continue
// //                                         }
// //                                     }
// //
// //                                 },
// //                                 Steal::Success(mut data) => {
// //                                     println!("THREAD 2: Tasks {}", tasks_done);
// //                                     tasks_done += 1;
// //                                     data.as_mut()();
// //                                 },
// //                                 Steal::Retry => {
// //                                     println!["LOCK COLLISION"];
// //                                     continue;
// //                                 },
// //                             }
// //                         } else {
// //                             println!("Executing Local tasks for thread 2");
// //                             let mut x = worker2.pop().unwrap();
// //                             x.as_mut()();
// //                         }
// //                     }
// //                 });
// //
// //                 h1.join().unwrap();
// //                 h2.join().unwrap();
// //             });
// //         }
// //     }
// // }
