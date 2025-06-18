use std::{
    collections::{LinkedList, VecDeque},
    convert::Infallible,
    marker::PhantomData,
};

use crate::collections::{InplaceVec, adapter::ContainerCommon};

pub trait QueueLike<T>: ContainerCommon {
    type PushError;

    fn push_back(&mut self, value: T) -> Result<(), Self::PushError>;

    fn pop_front(&mut self) -> Option<T>;

    fn front(&self) -> Option<&T>;

    fn front_mut(&mut self) -> Option<&mut T>;

    fn back(&self) -> Option<&T>;

    fn back_mut(&mut self) -> Option<&mut T>;
}

pub struct Queue<T, Container: QueueLike<T> = VecDeque<T>> {
    container: Container,
    _phantom_data: PhantomData<T>,
}

impl<T, Container: QueueLike<T>> Queue<T, Container> {
    #[inline]
    pub fn new(container: Container) -> Self {
        Self {
            container,
            _phantom_data: PhantomData,
        }
    }

    #[inline]
    pub fn inner(&self) -> &Container {
        &self.container
    }

    #[inline]
    pub fn inner_mut(&mut self) -> &mut Container {
        &mut self.container
    }

    #[inline]
    pub fn into_inner(self) -> Container {
        self.container
    }
}

impl<T, Container: QueueLike<T>> From<Container> for Queue<T, Container> {
    #[inline]
    fn from(value: Container) -> Self {
        Self::new(value)
    }
}

impl<T, Container: QueueLike<T> + Default> Default for Queue<T, Container> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T, Container: QueueLike<T> + std::fmt::Debug> std::fmt::Debug for Queue<T, Container> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queue")
            .field("container", &self.container)
            .finish()
    }
}

impl<T, Container: QueueLike<T>> ContainerCommon for Queue<T, Container> {
    #[inline]
    fn len(&self) -> usize {
        self.container.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.container.capacity()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.container.is_empty()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.container.is_full()
    }
}

impl<T, Container: QueueLike<T>> QueueLike<T> for Queue<T, Container> {
    type PushError = Container::PushError;

    #[inline]
    fn push_back(&mut self, value: T) -> Result<(), Self::PushError> {
        self.container.push_back(value)
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        self.container.pop_front()
    }

    #[inline]
    fn front(&self) -> Option<&T> {
        self.container.front()
    }

    #[inline]
    fn front_mut(&mut self) -> Option<&mut T> {
        self.container.front_mut()
    }

    #[inline]
    fn back(&self) -> Option<&T> {
        self.container.back()
    }

    #[inline]
    fn back_mut(&mut self) -> Option<&mut T> {
        self.container.back_mut()
    }
}

impl<T> QueueLike<T> for Vec<T> {
    type PushError = Infallible;

    #[inline]
    fn push_back(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push(value);
        Ok(())
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        (!self.is_empty()).then(|| self.remove(0))
    }

    #[inline]
    fn front(&self) -> Option<&T> {
        self.first()
    }

    #[inline]
    fn front_mut(&mut self) -> Option<&mut T> {
        self.first_mut()
    }

    #[inline]
    fn back(&self) -> Option<&T> {
        self.last()
    }

    #[inline]
    fn back_mut(&mut self) -> Option<&mut T> {
        self.last_mut()
    }
}

impl<T> QueueLike<T> for VecDeque<T> {
    type PushError = Infallible;

    #[inline]
    fn push_back(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push_back(value);
        Ok(())
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        self.pop_front()
    }

    #[inline]
    fn front(&self) -> Option<&T> {
        self.front()
    }

    #[inline]
    fn front_mut(&mut self) -> Option<&mut T> {
        self.front_mut()
    }

    #[inline]
    fn back(&self) -> Option<&T> {
        self.back()
    }

    #[inline]
    fn back_mut(&mut self) -> Option<&mut T> {
        self.back_mut()
    }
}

impl<T> QueueLike<T> for LinkedList<T> {
    type PushError = Infallible;

    #[inline]
    fn push_back(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push_back(value);
        Ok(())
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        self.pop_front()
    }

    #[inline]
    fn front(&self) -> Option<&T> {
        self.front()
    }

    #[inline]
    fn front_mut(&mut self) -> Option<&mut T> {
        self.front_mut()
    }

    #[inline]
    fn back(&self) -> Option<&T> {
        self.back()
    }

    #[inline]
    fn back_mut(&mut self) -> Option<&mut T> {
        self.back_mut()
    }
}

impl<T, const N: usize> QueueLike<T> for InplaceVec<T, N> {
    type PushError = T;

    #[inline]
    fn push_back(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push(value)
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        self.remove(0).ok()
    }

    #[inline]
    fn front(&self) -> Option<&T> {
        self.first()
    }

    #[inline]
    fn front_mut(&mut self) -> Option<&mut T> {
        self.first_mut()
    }

    #[inline]
    fn back(&self) -> Option<&T> {
        self.last()
    }

    #[inline]
    fn back_mut(&mut self) -> Option<&mut T> {
        self.last_mut()
    }
}
