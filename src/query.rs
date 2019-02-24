use std::iter::Repeat;
use std::iter::Take;
use std::iter::Zip;
use std::marker::PhantomData;
use std::slice::Iter;
use std::slice::IterMut;

use crate::*;

pub trait View<'a>: Sized + 'static {
    type Iter: Iterator + 'a;
    type Filter: ArchetypeFilter;

    fn fetch(chunk: &'a Chunk) -> Self::Iter;
    fn filter() -> Self::Filter;
}

pub trait Queryable<'a, World>: View<'a> {
    fn query(world: World) -> Query<'a, Self, <Self as View<'a>>::Filter, Passthrough>;
}

impl<'a, T: View<'a>> Queryable<'a, &'a mut World> for T {
    fn query(world: &'a mut World) -> Query<'a, Self, Self::Filter, Passthrough> {
        Query {
            world: world,
            view: PhantomData,
            arch_filter: Self::filter(),
            chunk_filter: Passthrough,
        }
    }
}

pub trait ReadOnly {}

impl<'a, T: View<'a> + ReadOnly> Queryable<'a, &'a World> for T {
    fn query(world: &'a World) -> Query<'a, Self, Self::Filter, Passthrough> {
        Query {
            world,
            view: PhantomData,
            arch_filter: Self::filter(),
            chunk_filter: Passthrough,
        }
    }
}

#[derive(Debug)]
pub struct Read<T: Component>(PhantomData<T>);

impl<T: Component> ReadOnly for Read<T> {}

impl<'a, T: Component> View<'a> for Read<T> {
    type Iter = Iter<'a, T>;
    type Filter = EntityDataFilter<T>;

    fn fetch(chunk: &'a Chunk) -> Self::Iter {
        unsafe { chunk.components().unwrap().iter() }
    }

    fn filter() -> Self::Filter {
        EntityDataFilter::new()
    }
}

#[derive(Debug)]
pub struct Write<T: Component>(PhantomData<T>);

impl<'a, T: Component> View<'a> for Write<T> {
    type Iter = IterMut<'a, T>;
    type Filter = EntityDataFilter<T>;

    fn fetch(chunk: &'a Chunk) -> Self::Iter {
        unsafe { chunk.components_mut().unwrap().iter_mut() }
    }

    fn filter() -> Self::Filter {
        EntityDataFilter::new()
    }
}

#[derive(Debug)]
pub struct Shared<T: SharedComponent>(PhantomData<T>);

impl<T: SharedComponent> ReadOnly for Shared<T> {}

impl<'a, T: SharedComponent> View<'a> for Shared<T> {
    type Iter = Take<Repeat<&'a T>>;
    type Filter = SharedDataFilter<T>;

    fn fetch(chunk: &'a Chunk) -> Self::Iter {
        unsafe {
            let data: &T = chunk.shared_component().unwrap();
            std::iter::repeat(data).take(chunk.len())
        }
    }

    fn filter() -> Self::Filter {
        SharedDataFilter::new()
    }
}

impl<'a, T1: View<'a>, T2: View<'a>> View<'a> for (T1, T2) {
    type Iter = Zip<T1::Iter, T2::Iter>;
    type Filter = And<T1::Filter, T2::Filter>;

    fn fetch(chunk: &'a Chunk) -> Self::Iter {
        T1::fetch(chunk).zip(T2::fetch(chunk))
    }

    fn filter() -> Self::Filter {
        And {
            a: T1::filter(),
            b: T2::filter(),
        }
    }
}

impl<T1: ReadOnly, T2: ReadOnly> ReadOnly for (T1, T2) {}

pub trait ArchetypeFilter {
    fn filter(&self, archetype: &Archetype) -> bool;
}

pub trait ChunkFilter {
    fn filter(&self, chunk: &Chunk) -> bool;
}

#[derive(Debug)]
pub struct Passthrough;

impl ArchetypeFilter for Passthrough {
    #[inline]
    fn filter(&self, _: &Archetype) -> bool {
        true
    }
}

impl ChunkFilter for Passthrough {
    #[inline]
    fn filter(&self, _: &Chunk) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct Not<F> {
    filter: F,
}

impl<F: ArchetypeFilter> ArchetypeFilter for Not<F> {
    #[inline]
    fn filter(&self, archetype: &Archetype) -> bool {
        !self.filter.filter(archetype)
    }
}

impl<F: ChunkFilter> ChunkFilter for Not<F> {
    #[inline]
    fn filter(&self, chunk: &Chunk) -> bool {
        !self.filter.filter(chunk)
    }
}

#[derive(Debug)]
pub struct And<A, B> {
    a: A,
    b: B,
}

impl<A: ArchetypeFilter, B: ArchetypeFilter> ArchetypeFilter for And<A, B> {
    #[inline]
    fn filter(&self, archetype: &Archetype) -> bool {
        self.a.filter(archetype) && self.b.filter(archetype)
    }
}

impl<A: ChunkFilter, B: ChunkFilter> ChunkFilter for And<A, B> {
    #[inline]
    fn filter(&self, chunk: &Chunk) -> bool {
        self.a.filter(chunk) && self.b.filter(chunk)
    }
}

#[derive(Debug)]
pub struct EntityDataFilter<T>(PhantomData<T>);

impl<T: Component> EntityDataFilter<T> {
    fn new() -> Self {
        EntityDataFilter(PhantomData)
    }
}

impl<T: Component> ArchetypeFilter for EntityDataFilter<T> {
    #[inline]
    fn filter(&self, archetype: &Archetype) -> bool {
        archetype.has_component::<T>()
    }
}

#[derive(Debug)]
pub struct SharedDataFilter<T>(PhantomData<T>);

impl<T: SharedComponent> SharedDataFilter<T> {
    fn new() -> Self {
        SharedDataFilter(PhantomData)
    }
}

impl<T: SharedComponent> ArchetypeFilter for SharedDataFilter<T> {
    #[inline]
    fn filter(&self, archetype: &Archetype) -> bool {
        archetype.has_shared::<T>()
    }
}

#[derive(Debug)]
pub struct SharedDataValueFilter<'a, T> {
    value: &'a T,
}

impl<'a, T: SharedComponent> SharedDataValueFilter<'a, T> {
    fn new(value: &'a T) -> Self {
        SharedDataValueFilter { value }
    }
}

impl<'a, T: SharedComponent> ChunkFilter for SharedDataValueFilter<'a, T> {
    #[inline]
    fn filter(&self, chunk: &Chunk) -> bool {
        unsafe { chunk.shared_component::<T>() }.map_or(false, |s| s == self.value)
    }
}

#[derive(Debug)]
pub struct Query<'a, V: View<'a>, A: ArchetypeFilter, C: ChunkFilter> {
    world: &'a World,
    view: PhantomData<V>,
    arch_filter: A,
    chunk_filter: C,
}

impl<'a, V: View<'a>, A: ArchetypeFilter, C: ChunkFilter> Query<'a, V, A, C>
where
    A: 'a,
    C: 'a,
{
    pub fn with_entity_data<T: Component>(self) -> Query<'a, V, And<A, EntityDataFilter<T>>, C> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: And {
                a: self.arch_filter,
                b: EntityDataFilter::new(),
            },
            chunk_filter: self.chunk_filter,
        }
    }

    pub fn without_entity_data<T: Component>(
        self,
    ) -> Query<'a, V, And<A, Not<EntityDataFilter<T>>>, C> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: And {
                a: self.arch_filter,
                b: Not {
                    filter: EntityDataFilter::new(),
                },
            },
            chunk_filter: self.chunk_filter,
        }
    }

    pub fn with_shared_data<T: SharedComponent>(
        self,
    ) -> Query<'a, V, And<A, SharedDataFilter<T>>, C> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: And {
                a: self.arch_filter,
                b: SharedDataFilter::new(),
            },
            chunk_filter: self.chunk_filter,
        }
    }

    pub fn without_shared_data<T: SharedComponent>(
        self,
    ) -> Query<'a, V, And<A, Not<SharedDataFilter<T>>>, C> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: And {
                a: self.arch_filter,
                b: Not {
                    filter: SharedDataFilter::new(),
                },
            },
            chunk_filter: self.chunk_filter,
        }
    }

    pub fn with_shared_data_value<'b, T: SharedComponent>(
        self,
        value: &'b T,
    ) -> Query<'a, V, A, And<C, SharedDataValueFilter<'b, T>>> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: self.arch_filter,
            chunk_filter: And {
                a: self.chunk_filter,
                b: SharedDataValueFilter::new(value),
            },
        }
    }

    pub fn without_shared_data_value<'b, T: SharedComponent>(
        self,
        value: &'b T,
    ) -> Query<'a, V, A, And<C, Not<SharedDataValueFilter<'b, T>>>> {
        Query {
            world: self.world,
            view: self.view,
            arch_filter: self.arch_filter,
            chunk_filter: And {
                a: self.chunk_filter,
                b: Not {
                    filter: SharedDataValueFilter::new(value),
                },
            },
        }
    }

    pub fn into_chunks(self) -> impl Iterator<Item = ChunkView<'a, V>> {
        let world = self.world;
        let arch = self.arch_filter;
        let chunk = self.chunk_filter;
        world
            .archetypes
            .iter()
            .filter(move |a| arch.filter(a))
            .flat_map(|a| a.chunks())
            .filter(move |c| chunk.filter(c))
            .map(|c| ChunkView {
                chunk: c,
                view: PhantomData,
            })
    }

    pub fn into_data(self) -> impl Iterator<Item = <<V as View<'a>>::Iter as Iterator>::Item> {
        self.into_chunks().flat_map(|mut c| c.data())
    }

    pub fn into_data_with_entities(
        self,
    ) -> impl Iterator<Item = (Entity, <<V as View<'a>>::Iter as Iterator>::Item)> {
        self.into_chunks().flat_map(|mut c| c.data_with_entities())
    }
}

#[derive(Debug)]
pub struct ChunkView<'a, V: View<'a>> {
    chunk: &'a Chunk,
    view: PhantomData<V>,
}

impl<'a, V: View<'a>> ChunkView<'a, V> {
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        unsafe { self.chunk.entities().iter() }
    }

    pub fn data(&mut self) -> V::Iter {
        V::fetch(self.chunk)
    }

    pub fn data_with_entities(
        &mut self,
    ) -> impl Iterator<Item = (Entity, <<V as View<'a>>::Iter as Iterator>::Item)> + 'a {
        unsafe {
            self.chunk
                .entities()
                .iter()
                .map(|e| *e)
                .zip(V::fetch(self.chunk))
        }
    }
}
