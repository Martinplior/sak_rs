use std::collections::{LinkedList, VecDeque};

use crate::collections::{InplaceDeque, InplaceVec};

pub trait ContainerCommon {
    fn len(&self) -> usize;

    fn capacity(&self) -> usize;

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }
}

impl<T> ContainerCommon for Vec<T> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> ContainerCommon for VecDeque<T> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> ContainerCommon for LinkedList<T> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        usize::MAX
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline]
    fn is_full(&self) -> bool {
        false
    }
}

impl<T, const N: usize> ContainerCommon for InplaceVec<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.is_full()
    }
}

impl<T, const N: usize> ContainerCommon for InplaceDeque<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.is_full()
    }
}
