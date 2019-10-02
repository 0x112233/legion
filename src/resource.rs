use crate::borrow::{AtomicRefCell, Exclusive, Ref, RefMapMut, Shared};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

pub enum ResourceAccessType {
    Read,
    Write,
}

pub trait Resource: 'static + Any + Send + Sync {}
impl<T> Resource for T where T: 'static + Any + Send + Sync {}

pub struct Read<'a, T: Resource> {
    inner: Ref<'a, Shared<'a>, &'a T>,
}
impl<'a, T: Resource> Deref for Read<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target { self.inner.deref() }
}

/// TODO: RefMut::map_into seems to have borrowing issues when used in conjunction with this trait
/// Hashmap. For now, I just pass the ref directly out, and perform the downcasting on Deref. This
/// is not optimal, but it is a hack for this code to work for now until someone smarter can fix.
pub struct Write<'a, T: Resource> {
    inner: RefMapMut<'a, Exclusive<'a>, &'a mut T>,
}
impl<'a, T: Resource> Deref for Write<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target { self.inner.deref() }
}

impl<'a, T: Resource> DerefMut for Write<'a, T> {
    fn deref_mut(&mut self) -> &mut T { self.inner.deref_mut() }
}

#[derive(Default)]
pub struct Resources {
    storage: HashMap<TypeId, AtomicRefCell<Box<dyn Resource>>>,
}

impl Resources {
    pub fn insert<T: Resource>(&mut self, value: T) -> Option<T> {
        self.storage
            .insert(TypeId::of::<T>(), AtomicRefCell::new(Box::new(value)));

        // TODO: Return the existing value
        None
    }

    pub fn get<T: Resource>(&self) -> Option<Read<'_, T>> {
        Some(Read {
            inner: self.storage.get(&TypeId::of::<T>())?.get().map(|v| {
                let r = v.as_ref();
                unsafe { std::mem::transmute(v) }
            }),
        })
    }

    pub fn get_mut<T: Resource>(&self) -> Option<Write<'_, T>> {
        Some(Write {
            inner: self
                .storage
                .get(&TypeId::of::<T>())?
                .get_mut()
                .map_into(|v| {
                    let r = v.as_ref();
                    unsafe { std::mem::transmute(v) }
                }),
        })
    }
}

pub trait Accessor: Send + Sync {
    type Output;

    fn reads(&self) -> Vec<TypeId>;
    fn writes(&self) -> Vec<TypeId>;
    fn fetch(resources: &Resources) -> Self::Output;
}

impl Accessor for () {
    type Output = ();

    fn reads(&self) -> Vec<TypeId> { vec![] }
    fn writes(&self) -> Vec<TypeId> { vec![] }
    fn fetch(_resources: &Resources) {}
}

macro_rules! impl_resource_tuple {
    ( $( $ty: ident ),* ) => {
        impl<$( $ty: Resource ),*> Accessor for ($( $ty, )*)
        {
            type Output = ($( $ty, )*);

                fn reads(&self) -> Vec<TypeId> { vec![$( TypeId::of::<$ty>()),*] }
                fn writes(&self) -> Vec<TypeId> { vec![$( TypeId::of::<$ty>()),*] }
                fn fetch(_: &Resources) -> ($( $ty, )*) { unimplemented!() }
        }
    };
}

impl_resource_tuple!(A);
impl_resource_tuple!(A, B);
impl_resource_tuple!(A, B, C);
impl_resource_tuple!(A, B, C, D);
impl_resource_tuple!(A, B, C, D, E);
impl_resource_tuple!(A, B, C, D, E, F);
impl_resource_tuple!(A, B, C, D, E, F, G);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_read_write_test() {
        let _ = env_logger::builder().is_test(true).try_init();

        struct TestOne {
            value: String,
        }

        struct TestTwo {
            value: String,
        }

        let mut resources = Resources::default();
        resources.insert(TestOne {
            value: "poop".to_string(),
        });

        resources.insert(TestTwo {
            value: "balls".to_string(),
        });

        assert_eq!(resources.get::<TestOne>().unwrap().value, "poop");
        assert_eq!(resources.get::<TestTwo>().unwrap().value, "balls");
    }
}
