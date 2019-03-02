mod borrows;
mod query;
mod storage;

pub use crate::borrows::*;
pub use crate::query::*;
pub use crate::storage::*;

use parking_lot::Mutex;
use slog::{debug, info, o, trace, Drain};
use std::any::TypeId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use std::iter::Peekable;
use std::num::Wrapping;
use std::sync::Arc;

pub type EntityIndex = u16;
pub type EntityVersion = Wrapping<u16>;
pub type ComponentID = u16;
pub type ChunkID = u16;
pub type ArchetypeID = u16;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Entity {
    index: EntityIndex,
    version: EntityVersion,
}

impl Entity {
    pub fn new(index: EntityIndex, version: EntityVersion) -> Entity {
        Entity {
            index: index,
            version: version,
        }
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}#{}", self.index, self.version)
    }
}

#[derive(Debug)]
pub struct Universe {
    name: String,
    logger: slog::Logger,
    allocator: Arc<Mutex<BlockAllocator>>,
}

impl Universe {
    pub fn new<L: Into<Option<slog::Logger>>>(logger: L) -> Self {
        let name = names::Generator::default().next().unwrap();
        let logger = logger
            .into()
            .unwrap_or(slog::Logger::root(slog_stdlog::StdLog.fuse(), o!()))
            .new(o!("universe" => name.clone()));

        info!(logger, "starting universe");
        Universe {
            name,
            logger,
            allocator: Arc::from(Mutex::new(BlockAllocator::new())),
        }
    }

    pub fn create_world(&self) -> World {
        World::new(
            self.logger.clone(),
            EntityAllocator::new(self.allocator.clone()),
        )
    }
}

#[derive(Debug)]
struct BlockAllocator {
    allocated: usize,
    free: Vec<EntityBlock>,
}

impl BlockAllocator {
    const BLOCK_SIZE: usize = 1024;

    pub fn new() -> Self {
        BlockAllocator {
            allocated: 0,
            free: Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> EntityBlock {
        if let Some(block) = self.free.pop() {
            block
        } else {
            let block = EntityBlock::new(self.allocated as EntityIndex, BlockAllocator::BLOCK_SIZE);
            self.allocated += BlockAllocator::BLOCK_SIZE;
            block
        }
    }

    pub fn free(&mut self, block: EntityBlock) {
        self.free.push(block);
    }
}

#[derive(Debug)]
struct EntityBlock {
    start: EntityIndex,
    len: usize,
    versions: Vec<EntityVersion>,
    free: Vec<EntityIndex>,
}

impl EntityBlock {
    pub fn new(start: u16, len: usize) -> EntityBlock {
        EntityBlock {
            start: start,
            len: len,
            versions: Vec::new(),
            free: Vec::new(),
        }
    }

    fn index(&self, index: EntityIndex) -> usize {
        (index - self.start) as usize
    }

    pub fn is_alive(&self, entity: &Entity) -> Option<bool> {
        if entity.index >= self.start {
            let i = self.index(entity.index);
            self.versions.get(i).map(|v| *v == entity.version)
        } else {
            None
        }
    }

    pub fn allocate(&mut self) -> Option<Entity> {
        if let Some(index) = self.free.pop() {
            let i = self.index(index);
            Some(Entity::new(index, self.versions[i]))
        } else if self.versions.len() < self.len {
            let index = self.start + self.versions.len() as EntityIndex;
            self.versions.push(Wrapping(1));
            Some(Entity::new(index, Wrapping(1)))
        } else {
            None
        }
    }

    pub fn free(&mut self, entity: Entity) -> Option<bool> {
        if let Some(alive) = self.is_alive(&entity) {
            let i = self.index(entity.index);
            self.versions[i] += Wrapping(1);
            self.free.push(entity.index);
            Some(alive)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct EntityAllocator {
    allocator: Arc<Mutex<BlockAllocator>>,
    blocks: Vec<EntityBlock>,
    entity_buffer: Vec<Entity>,
}

impl EntityAllocator {
    fn new(allocator: Arc<Mutex<BlockAllocator>>) -> Self {
        EntityAllocator {
            allocator: allocator,
            blocks: Vec::new(),
            entity_buffer: Vec::new(),
        }
    }

    pub fn is_alive(&self, entity: &Entity) -> bool {
        self.blocks
            .iter()
            .filter_map(|b| b.is_alive(entity))
            .nth(0)
            .unwrap_or(false)
    }

    pub fn create_entity(&mut self) -> Entity {
        let entity = if let Some(entity) = self
            .blocks
            .iter_mut()
            .rev()
            .filter_map(|b| b.allocate())
            .nth(0)
        {
            entity
        } else {
            let mut block = self.allocator.lock().allocate();
            let entity = block.allocate().unwrap();
            self.blocks.push(block);
            entity
        };

        self.entity_buffer.push(entity.clone());
        entity
    }

    pub fn delete_entity(&mut self, entity: Entity) -> bool {
        self.blocks
            .iter_mut()
            .filter_map(|b| b.free(entity))
            .nth(0)
            .unwrap_or(false)
    }

    pub fn allocation_buffer(&self) -> &[Entity] {
        self.entity_buffer.as_slice()
    }

    pub fn clear_allocation_buffer(&mut self) {
        self.entity_buffer.clear();
    }
}

impl Drop for EntityAllocator {
    fn drop(&mut self) {
        for block in self.blocks.drain(..) {
            self.allocator.lock().free(block);
        }
    }
}

#[derive(Debug)]
pub struct World {
    name: String,
    logger: slog::Logger,
    allocator: EntityAllocator,
    archetypes: Vec<Archetype>,
    entities: HashMap<Entity, (ArchetypeID, ChunkID, ComponentID)>,
}

impl World {
    fn new(logger: slog::Logger, allocator: EntityAllocator) -> Self {
        let name = names::Generator::default().next().unwrap();
        let logger = logger.new(o!("world" => name.clone()));

        info!(logger, "starting world");
        World {
            name,
            logger,
            allocator: allocator,
            archetypes: Vec::new(),
            entities: HashMap::new(),
        }
    }

    pub fn is_alive(&self, entity: &Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    pub fn insert_from<S, T>(&mut self, shared: S, components: T) -> &[Entity]
    where
        S: SharedDataSet,
        T: IntoIterator,
        T::Item: ComponentDataSet,
        IterComponentSource<T::IntoIter, T::Item>: ComponentSource,
    {
        let source = T::Item::component_source(components.into_iter());
        self.insert(shared, source)
    }

    pub fn insert<S, T>(&mut self, shared: S, mut components: T) -> &[Entity]
    where
        S: SharedDataSet,
        T: ComponentSource,
    {
        let allocator = &mut self.allocator;
        let entities = &mut self.entities;

        allocator.clear_allocation_buffer();

        // find or create archetype
        let (arch_id, archetype) =
            World::archetype(&self.logger, &mut self.archetypes, &shared, &components);

        // insert components into chunks
        while !components.is_empty() {
            // find or create chunk
            let (chunk_id, chunk) = archetype.get_or_create_chunk(&shared, &components);

            // insert as many components as we can into the chunk
            let allocated = components.write(chunk, allocator);

            // record new entity locations
            let start = unsafe { chunk.entities().len() - allocated };
            let added = unsafe { chunk.entities().iter().enumerate().skip(start) };
            for (i, e) in added {
                let comp_id = i as ComponentID;
                entities.insert(*e, (arch_id, chunk_id, comp_id));
            }

            trace!(
                self.logger,
                "appended {entity_count} entities into chunk",
                entity_count = allocated;
                "archetype_id" => arch_id,
                "chunk_id" => chunk_id
            );
        }

        trace!(
            self.logger,
            "inserted {entity_count_added} entities",
            entity_count_added = self.allocator.allocation_buffer().len();
            "archetype_id" => arch_id
        );

        self.allocator.allocation_buffer()
    }

    pub fn delete(&mut self, entity: Entity) -> bool {
        let deleted = self.allocator.delete_entity(entity);

        if deleted {
            // lookup entity location
            let ids = self
                .entities
                .get(&entity)
                .map(|(archetype_id, chunk_id, component_id)| {
                    (*archetype_id, *chunk_id, *component_id)
                });

            // swap remove with last entity in chunk
            let swapped = ids.and_then(|(archetype_id, chunk_id, component_id)| {
                self.archetypes
                    .get_mut(archetype_id as usize)
                    .and_then(|archetype| archetype.chunk_mut(chunk_id))
                    .and_then(|chunk| unsafe { chunk.remove(component_id) })
            });

            // record swapped entity's new location
            if let Some(swapped) = swapped {
                self.entities.insert(swapped, ids.unwrap());
            }
        }

        deleted
    }

    pub fn component<'a, T: EntityData>(&'a self, entity: Entity) -> Option<Borrowed<'a, T>> {
        self.entities
            .get(&entity)
            .and_then(|(archetype_id, chunk_id, component_id)| {
                self.archetypes
                    .get(*archetype_id as usize)
                    .and_then(|archetype| archetype.chunk(*chunk_id))
                    .and_then(|chunk| chunk.entity_data::<T>())
                    .and_then(|vec| vec.single(*component_id as usize))
            })
    }

    pub fn component_mut<T: EntityData>(&mut self, entity: Entity) -> Option<&mut T> {
        let entities = &self.entities;
        let archetypes = &self.archetypes;
        entities
            .get(&entity)
            .and_then(|(archetype_id, chunk_id, component_id)| {
                archetypes
                    .get(*archetype_id as usize)
                    .and_then(|archetype| archetype.chunk(*chunk_id))
                    .and_then(|chunk| unsafe { chunk.entity_data_unchecked::<T>() })
                    .and_then(|vec| vec.get_mut(*component_id as usize))
            })
    }

    pub fn shared<T: SharedData>(&self, entity: Entity) -> Option<&T> {
        self.entities
            .get(&entity)
            .and_then(|(archetype_id, chunk_id, _)| {
                self.archetypes
                    .get(*archetype_id as usize)
                    .and_then(|archetype| archetype.chunk(*chunk_id))
                    .and_then(|chunk| unsafe { chunk.shared_component::<T>() })
            })
    }

    fn archetype<'a, S: SharedDataSet, C: ComponentSource>(
        logger: &slog::Logger,
        archetypes: &'a mut Vec<Archetype>,
        shared: &S,
        components: &C,
    ) -> (ArchetypeID, &'a mut Archetype) {
        match archetypes
            .iter()
            .enumerate()
            .filter(|(_, a)| components.is_archetype_match(a) && shared.is_archetype_match(a))
            .map(|(i, _)| i)
            .next()
        {
            Some(i) => (i as ArchetypeID, unsafe { archetypes.get_unchecked_mut(i) }),
            None => {
                let archetype_id = archetypes.len() as ArchetypeID;
                let logger = logger.new(o!("archetype_id" => archetype_id));
                let archetype = Archetype::new(logger.clone(), components.types(), shared.types());
                archetypes.push(archetype);

                debug!(logger, "allocated archetype");

                (archetype_id, archetypes.last_mut().unwrap())
            }
        }
    }
}

pub trait SharedDataSet {
    fn is_archetype_match(&self, archetype: &Archetype) -> bool;
    fn is_chunk_match(&self, chunk: &Chunk) -> bool;
    fn configure_chunk(&self, chunk: &mut ChunkBuilder);
    fn types(&self) -> HashSet<TypeId>;
}

pub trait ComponentDataSet: Sized {
    fn component_source<T>(source: T) -> IterComponentSource<T, Self>
    where
        T: Iterator<Item = Self>;
}

pub trait ComponentSource {
    fn is_archetype_match(&self, archetype: &Archetype) -> bool;
    fn configure_chunk(&self, chunk: &mut ChunkBuilder);
    fn types(&self) -> HashSet<TypeId>;
    fn is_empty(&mut self) -> bool;
    fn write<'a>(&mut self, chunk: &'a mut Chunk, allocator: &mut EntityAllocator) -> usize;
}

impl SharedDataSet for () {
    fn is_archetype_match(&self, archetype: &Archetype) -> bool {
        archetype.shared.len() == 0
    }

    fn is_chunk_match(&self, _: &Chunk) -> bool {
        true
    }

    fn configure_chunk(&self, _: &mut ChunkBuilder) {}

    fn types(&self) -> HashSet<TypeId> {
        HashSet::new()
    }
}

macro_rules! impl_shared_data_set {
    ( $arity: expr; $( $ty: ident ),* ) => {
        impl<$( $ty ),*> SharedDataSet for ($( $ty, )*)
        where $( $ty: SharedData ),*
        {
            fn is_archetype_match(&self, archetype: &Archetype) -> bool {
                archetype.shared.len() == $arity &&
                $( archetype.shared.contains(&TypeId::of::<$ty>()) )&&*
            }

            fn is_chunk_match(&self, chunk: &Chunk) -> bool {
                unsafe {
                    #![allow(non_snake_case)]
                    let ($($ty,)*) = self;
                    $(
                        (*chunk.shared_component::<$ty>().unwrap() == *$ty)
                    )&&*
                }
            }

            fn configure_chunk(&self, chunk: &mut ChunkBuilder) {
                #![allow(non_snake_case)]
                let ($( ref $ty, )*) = self;
                $( chunk.register_shared($ty.clone()); )*
            }

            fn types(&self) -> HashSet<TypeId> {
                [$( TypeId::of::<$ty>() ),*].iter().cloned().collect()
            }
        }
    }
}

impl_shared_data_set!(1; A);
impl_shared_data_set!(2; A, B);
impl_shared_data_set!(3; A, B, C);
impl_shared_data_set!(4; A, B, C, D);
impl_shared_data_set!(5; A, B, C, D, E);

pub struct IterComponentSource<T: Iterator<Item = K>, K> {
    source: Peekable<T>,
}

macro_rules! impl_component_source {
    ( $arity: expr; $( $ty: ident => $id: ident ),* ) => {
        impl<$( $ty ),*> ComponentDataSet for ($( $ty, )*)
        where $( $ty: EntityData ),*
        {
            fn component_source<T>(source: T) -> IterComponentSource<T, Self>
                where T: Iterator<Item=Self>
            {
                IterComponentSource::<T, Self> { source: source.peekable() }
            }
        }

        impl<I, $( $ty ),*> ComponentSource for IterComponentSource<I, ($( $ty, )*)>
        where I: Iterator<Item=($( $ty, )*)>,
              $( $ty: EntityData ),*
        {
            fn types(&self) -> HashSet<TypeId> {
                [$( TypeId::of::<$ty>() ),*].iter().cloned().collect()
            }

            fn is_archetype_match(&self, archetype: &Archetype) -> bool {
                archetype.components.len() == $arity &&
                $(
                    archetype.components.contains(&TypeId::of::<$ty>())
                )&&*
            }

            fn configure_chunk(&self, chunk: &mut ChunkBuilder) {
                $(
                    chunk.register_component::<$ty>();
                )*
            }

            fn is_empty(&mut self) -> bool {
                self.source.peek().is_none()
            }

            fn write<'a>(&mut self, chunk: &'a mut Chunk, allocator: &mut EntityAllocator) -> usize {
                #![allow(non_snake_case)]
                let mut count = 0;

                unsafe {
                    let entities = chunk.entities_unchecked();
                    $(
                        let $ty = chunk.entity_data_unchecked::<$ty>().unwrap();
                    )*

                    while let Some(($( $id, )*)) = { if chunk.is_full() { None } else { self.source.next() } } {
                        let entity = allocator.create_entity();
                        entities.push(entity);
                        $(
                            $ty.push($id);
                        )*
                        count += 1;
                    }
                }

                chunk.validate();
                count
            }
        }
    }
}

impl_component_source!(1; A => a);
impl_component_source!(2; A => a, B => b);
impl_component_source!(3; A => a, B => b, C => c);
impl_component_source!(4; A => a, B => b, C => c, D => d);
impl_component_source!(5; A => a, B => b, C => c, D => d, E => e);

pub trait EntityData: Send + Sync + Sized + Debug + 'static {}
pub trait SharedData: Send + Sync + Sized + PartialEq + Clone + Debug + 'static {}

impl<T: Send + Sync + Sized + Debug + 'static> EntityData for T {}
impl<T: Send + Sized + PartialEq + Clone + Sync + Debug + 'static> SharedData for T {}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn create_universe() {
        Universe::new(None);
    }

    #[test]
    fn create_world() {
        let universe = Universe::new(None);
        universe.create_world();
    }

    #[test]
    fn create_entity() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        allocator.create_entity();
    }

    #[test]
    fn create_entity_many() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));

        for _ in 0..512 {
            allocator.create_entity();
        }
    }

    #[test]
    fn create_entity_many_blocks() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));

        for _ in 0..3000 {
            allocator.create_entity();
        }
    }

    #[test]
    fn create_entity_recreate() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));

        for _ in 0..3 {
            let entities: Vec<Entity> = (0..512).map(|_| allocator.create_entity()).collect();
            for e in entities {
                allocator.delete_entity(e);
            }
        }
    }

    #[test]
    fn is_alive_allocated() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = allocator.create_entity();

        assert_eq!(true, allocator.is_alive(&entity));
    }

    #[test]
    fn is_alive_unallocated() {
        let allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = Entity::new(10 as EntityIndex, Wrapping(10));

        assert_eq!(false, allocator.is_alive(&entity));
    }

    #[test]
    fn is_alive_killed() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = allocator.create_entity();
        allocator.delete_entity(entity);

        assert_eq!(false, allocator.is_alive(&entity));
    }

    #[test]
    fn delete_entity_was_alive() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = allocator.create_entity();

        assert_eq!(true, allocator.delete_entity(entity));
    }

    #[test]
    fn delete_entity_was_dead() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = allocator.create_entity();
        allocator.delete_entity(entity);

        assert_eq!(false, allocator.delete_entity(entity));
    }

    #[test]
    fn delete_entity_was_unallocated() {
        let mut allocator = EntityAllocator::new(Arc::from(Mutex::new(BlockAllocator::new())));
        let entity = Entity::new(10 as EntityIndex, Wrapping(10));

        assert_eq!(false, allocator.delete_entity(entity));
    }

    #[test]
    fn multiple_allocators_unique_ids() {
        let blocks = Arc::from(Mutex::new(BlockAllocator::new()));
        let mut allocator_a = EntityAllocator::new(blocks.clone());
        let mut allocator_b = EntityAllocator::new(blocks.clone());

        let mut entities_a = HashSet::<Entity>::new();
        let mut entities_b = HashSet::<Entity>::new();

        for _ in 0..5 {
            entities_a.extend((0..1500).map(|_| allocator_a.create_entity()));
            entities_b.extend((0..1500).map(|_| allocator_b.create_entity()));
        }

        assert_eq!(true, entities_a.is_disjoint(&entities_b));

        for e in entities_a {
            assert_eq!(true, allocator_a.is_alive(&e));
            assert_eq!(false, allocator_b.is_alive(&e));
        }

        for e in entities_b {
            assert_eq!(false, allocator_a.is_alive(&e));
            assert_eq!(true, allocator_b.is_alive(&e));
        }
    }
}
