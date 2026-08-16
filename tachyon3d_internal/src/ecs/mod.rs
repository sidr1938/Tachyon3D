use std::any::{Any, TypeId};
use std::cmp::{max, PartialEq};
use std::collections::HashMap;
use std::fmt::Debug;
use std::num::NonZeroU32;
use fxhash::FxHashMap;
use crate::app::AppT3D;
use crate::app::plugin::Installation;
// How many bits we want for the generation, this is a hard limit to allow for some memory optimizations
const GEN_BITS: u8 = 32;
const COMPONENTS: usize = 64;
const COMPONENT_BITSET_LENGTH: usize = 64;
const COMPONENT_BITSETS: usize = COMPONENTS / COMPONENT_BITSET_LENGTH;

// NOTE:
// u8/i8/bool - 1 byte
// u16/i16 - 2 bytes
// u32/f32/i32 - 4 byte
// u64/f64/i64 - 8 bytes

// Get 1s for all slots that gen_bits exists and 0s for all slots that index exists
// Masking is applied via an & operation, where only bits that are 1 survive,
// this helps filter out the index

pub struct ECSPlugin;
#[derive(Copy, Clone)]
#[repr(transparent)]
#[derive(Debug)]
pub struct Entity {
    pub key: u64
}

struct Bitset {
    set: [u64; COMPONENT_BITSETS]
}

impl Bitset {
    fn new() -> Self {
        Bitset {
            set: [0u64; COMPONENT_BITSETS],
        }
    }
}
pub struct World {
    entities: Vec<Entity>,
    entity_pointers: EntityPointers,

    bitsets: Vec<Bitset>,
    component_ids: FxHashMap<TypeId, u32>,
    archetypes: Vec<Archetype>,

    free_entities: u32,
    partition: u32,
}

trait Component {}
#[derive(Debug)]
pub struct EntityPointers {
    internal: Vec<u32>,
}

impl EntityPointers {
    fn new(capacity: usize) -> Self {
        EntityPointers {
            internal: vec![0; capacity],
        }
    }
    fn insert(&mut self, sparse_loc: u32, dense_loc: u32) {
        if let Some(ptr) = self.internal.get_mut(sparse_loc as usize) {
            // Set the dense location
            *ptr = dense_loc;
        } else {
            // Add a new location if the entities exceed the initial capacity
            self.internal.push(dense_loc);
        }
    }
}
impl Installation for ECSPlugin {
    fn install_plugin(self, app: &mut AppT3D)
    where
        Self: Sized,
    {
        eprintln!["* [INFO]: Installing ECS"];
        app.install(World::new(100));
    }
}


impl Installation for World {}
#[derive(Debug)]
pub enum ECSError {
    EntityNotFound,
    OutOfBounds
}

impl PartialEq for Entity {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

struct Archetype {
    entities: Vec<Entity>,
    data: Vec<Box<dyn Any>>,
    sparse: Vec<u32>

}
impl Archetype {
    fn new() -> Self {
        Archetype {
            entities: Vec::new(),
            data: Default::default(),
            sparse: Vec::new(),
        }
    }
}

impl World {
    pub fn new(capacity: usize) -> Self {
        World {
            entities: Vec::new(),
            entity_pointers: EntityPointers::new(capacity),
            //data: Default::default(),
            bitsets: Vec::new(),
            component_ids: Default::default(),
            archetypes: Vec::new(),

            partition: 0,
            free_entities: 0,
        }
    }
    pub fn spawn(&mut self) -> Entity {
        let entity;
        if self.free_entities == 0 {
            // Create a new entity
            let dense_loc = self.entities.len() as u32;
            // The sparse index is the dense index
            self.entity_pointers.insert(dense_loc, dense_loc);
            // Send the new entity
            entity = Entity::new(dense_loc as u64, 0u64);
            self.entities.push(entity);
        } else {
            // 0 allocations needed
            entity = self.entities[self.partition as usize];
            self.free_entities -= 1;
        }
        self.partition += 1;
        entity
    }
    pub fn add_comp<T: Component + 'static, U: Component + 'static>(&mut self, entity: Entity, comp: T, comp2: U) {
        if !self.component_ids.contains_key(&comp.type_id()) {
            self.component_ids.insert(comp.type_id(), self.component_ids.len() as u32);
        }
        if !self.component_ids.contains_key(&comp2.type_id()) {
            self.component_ids.insert(comp2.type_id(), self.component_ids.len() as u32);
        }

        let off1 = self.component_ids.get(&comp.type_id()).unwrap();
        let off2 = self.component_ids.get(&comp2.type_id()).unwrap();
        dbg![off1];
        dbg!(off2);
        let mut set = 0u64;
        set |= 1 << off1;
        set |= 1 << off2;

        println!("Bitset binary: {:064b}", set);
        for (arch_idx, bitset) in self.bitsets.iter().enumerate() {
            if bitset.set[0] == set {
                self.archetypes[arch_idx].entities.push(entity);
                let a = self.archetypes[arch_idx].sparse[*off1 as usize];
                let b = self.archetypes[arch_idx].sparse[*off2 as usize];
                let array1 = self.archetypes[arch_idx].data[a as usize].downcast_mut::<Vec<T>>().unwrap();
                array1.push(comp);
                let array2 = self.archetypes[arch_idx].data[b as usize].downcast_mut::<Vec<U>>().unwrap();
                array2.push(comp2);
                return;
            }
        }
        self.archetypes.push(Archetype::new());
        let handle = self.archetypes.last_mut().unwrap();
        handle.entities.push(entity);
        handle.sparse = vec![0; max(*off1 as usize, *off2 as usize) + 1];
        handle.sparse[*off1 as usize] = 0;
        handle.sparse[*off2 as usize] = 1;
        self.bitsets.push(Bitset {
            set: [set],
        });
        self.archetypes.last_mut().unwrap().data.push(Box::new(Vec::from([comp])));
        self.archetypes.last_mut().unwrap().data.push(Box::new(Vec::from([comp2])));
    }

    pub fn despawn(&mut self, entity: Entity) -> Result<(), ECSError> {
        let dense_loc = self.entity_pointers.internal[entity.id()];
        // Test if the entity being accessed is the same in the list, if not its stale
        if entity == *self.entities.get(dense_loc as usize).ok_or(ECSError::OutOfBounds)? {
            // done for every deallocation
            self.partition -= 1;
            self.free_entities += 1;
            self.entities[dense_loc as usize] = entity.increment();
            if dense_loc != self.partition {
                // We swap this into the last into the despawned entities location so we need to update its sparse index
                let sparse_boundary_index = self.entities[self.partition as usize].id();
                self.entity_pointers.internal.swap(entity.id(), sparse_boundary_index);
                self.entities.swap(dense_loc as usize, (self.partition) as usize);
            }
            return Ok(())
        }
        Err(ECSError::EntityNotFound)
    }
}


impl Entity {
    fn new(id: u64, generation: u64) -> Self {
        Entity { key: (id << GEN_BITS) | generation }
    }
    fn id(&self) -> usize {
        (self.key >> GEN_BITS) as usize
    }
    fn generation(&self) -> usize {
        (self.key as u32) as usize
    }
    fn increment(mut self) -> Self {
        self.key = (self.key & 0xFFFF_FFFF_0000_0000) | ((self.key as u32).wrapping_add(1) as u64);
        self
    }
}


mod tests {
    use tachyon3d_internal::ecs::Component;
    use crate::ecs::World;
    struct Health(u32);
    struct Velocity {
        x: u32,
        y: u32
    }
    impl Velocity {
        fn increase(&mut self) {
            self.x += 1;
            self.y += 3;
        }
        fn new() -> Self {
            Velocity {
                x: 0,
                y: 0,
            }
        }
    }
    impl Health {
        fn lessen(&mut self) {
           self.0 -= 10;
        }
    }
    impl Component for Health {}
    impl Component for Velocity {}
    #[test]
    fn test() {
        let mut s = World::new(1);
        let handle1 = s.spawn();
        let handle2 = s.spawn();
        let handle3 = s.spawn();
        s.despawn(handle1).unwrap();
        let handle1 = s.spawn();
        s.despawn(handle1).unwrap();
        let handle1 = s.spawn();
        s.despawn(handle1).unwrap();
        let handle1 = s.spawn();


        //let handle4 = s.spawn();
        s.despawn(handle1).expect("Error");
        let handle = s.spawn();
        s.despawn(handle).expect("Error");
        let handle = s.spawn();
        s.despawn(handle).expect("Error");
        s.despawn(handle2).expect("Error");
        s.despawn(handle3).expect("Error");
        let a = s.spawn();
        let b = s.spawn();
        let c = s.spawn();
        s.add_comp(a, Health(45), Velocity::new());
        s.add_comp(b, Health(45), Velocity::new());
        s.add_comp(c, Health(45), Velocity::new());
        dbg![&s.archetypes.len()];
        dbg![&s.archetypes[0].data.len()];
        dbg![&s.archetypes[0].entities];
        // for i in s.archetypes[0].data.iter_mut() {
        //     dbg![1];
        //     let s = i.downcast_mut::<Health>().unwrap();
        //     s.0.lessen();
        //     s.1.increase();
        // }
    }
    #[test]
    fn test_entity_spawn_delete() {
        //eprintln!["* [INFO]: Loading WorldPlugin"];
        let mut world = World::new(2);
        let e1 = world.spawn();
        let e2 = world.spawn();
        // Spawning
        assert_ne!(e1.id(), e2.id());

        // Entity destruction and error return on stale handle
        assert!(world.despawn(e1).is_ok());
        assert!(world.despawn(e1).is_err());

        // Old entity reuse
        let e3 = world.spawn();
        assert_eq!(e1.id(), e3.id());
        assert_eq!(e3.generation(), 1);
        let a = world.spawn();
        let b = world.spawn();
        world.spawn();
    }
}