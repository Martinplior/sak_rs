use std::{
    any::TypeId,
    collections::HashSet,
    marker::PhantomData,
    sync::{LazyLock, Mutex},
};

static TYPE_SET: LazyLock<Mutex<HashSet<TypeId>>> = LazyLock::new(Default::default);

/// A thread-safe singleton cell for singleton pattern. It ensures that only one instance of a type can be created.
#[derive(Debug)]
pub struct SingletonCell<T: 'static>(T, PhantomData<fn(T) -> T>);

impl<T: 'static> SingletonCell<T> {
    const TYPE_ID: TypeId = TypeId::of::<T>();

    #[inline]
    pub fn new(value: T) -> Option<Self> {
        TYPE_SET
            .lock()
            .expect("unreachable")
            .insert(Self::TYPE_ID)
            .then(|| Self(value, Default::default()))
    }
}

impl<T: 'static> std::ops::Deref for SingletonCell<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> std::ops::DerefMut for SingletonCell<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: 'static> Drop for SingletonCell<T> {
    fn drop(&mut self) {
        let removed = TYPE_SET.lock().expect("unreachable").remove(&Self::TYPE_ID);
        debug_assert!(removed, "SingletonCell duplicated drop");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1() {
        fn f1(_: &()) {}
        fn f2(_: &'static ()) {}

        let _other_ring: SingletonCell<fn(&())> = SingletonCell::new(f1 as _).unwrap();
        let _the_one_ring: SingletonCell<fn(&'static ())> = SingletonCell::new(f2 as _).unwrap();

        // let fake_one_ring: SingletonCell<fn(&'static ())> = _other_ring; // invariant
    }
}
