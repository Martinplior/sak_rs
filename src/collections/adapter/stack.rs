use std::{
    collections::{LinkedList, VecDeque},
    convert::Infallible,
    marker::PhantomData,
};

use crate::collections::{InplaceVec, adapter::ContainerCommon};

pub trait StackLike<T>: ContainerCommon {
    type PushError;

    fn push(&mut self, value: T) -> Result<(), Self::PushError>;

    fn pop(&mut self) -> Option<T>;

    fn top(&self) -> Option<&T>;

    fn top_mut(&mut self) -> Option<&mut T>;
}

pub struct Stack<T, Container: StackLike<T> = Vec<T>> {
    container: Container,
    _phanom_data: PhantomData<T>,
}

impl<T, Container: StackLike<T>> Stack<T, Container> {
    #[inline]
    pub fn new(container: Container) -> Self {
        Self {
            container,
            _phanom_data: PhantomData,
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

impl<T, Container: StackLike<T>> From<Container> for Stack<T, Container> {
    #[inline]
    fn from(value: Container) -> Self {
        Self::new(value)
    }
}

impl<T, Container: StackLike<T> + Default> Default for Stack<T, Container> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T, Container: StackLike<T> + std::fmt::Debug> std::fmt::Debug for Stack<T, Container> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stack")
            .field("container", &self.container)
            .finish()
    }
}

impl<T, Container: StackLike<T>> ContainerCommon for Stack<T, Container> {
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

impl<T, Container: StackLike<T>> StackLike<T> for Stack<T, Container> {
    type PushError = Container::PushError;

    #[inline]
    fn push(&mut self, value: T) -> Result<(), Self::PushError> {
        self.container.push(value)
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        self.container.pop()
    }

    #[inline]
    fn top(&self) -> Option<&T> {
        self.container.top()
    }

    #[inline]
    fn top_mut(&mut self) -> Option<&mut T> {
        self.container.top_mut()
    }
}

impl<T> StackLike<T> for Vec<T> {
    type PushError = Infallible;

    #[inline]
    fn push(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push(value);
        Ok(())
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    #[inline]
    fn top(&self) -> Option<&T> {
        self.last()
    }

    #[inline]
    fn top_mut(&mut self) -> Option<&mut T> {
        self.last_mut()
    }
}

impl<T> StackLike<T> for VecDeque<T> {
    type PushError = Infallible;

    #[inline]
    fn push(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push_back(value);
        Ok(())
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        self.pop_back()
    }

    #[inline]
    fn top(&self) -> Option<&T> {
        self.back()
    }

    #[inline]
    fn top_mut(&mut self) -> Option<&mut T> {
        self.back_mut()
    }
}

impl<T> StackLike<T> for LinkedList<T> {
    type PushError = Infallible;

    #[inline]
    fn push(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push_back(value);
        Ok(())
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        self.pop_back()
    }

    #[inline]
    fn top(&self) -> Option<&T> {
        self.back()
    }

    #[inline]
    fn top_mut(&mut self) -> Option<&mut T> {
        self.back_mut()
    }
}

impl<T, const N: usize> StackLike<T> for InplaceVec<T, N> {
    type PushError = T;

    #[inline]
    fn push(&mut self, value: T) -> Result<(), Self::PushError> {
        self.push(value)
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    #[inline]
    fn top(&self) -> Option<&T> {
        self.last()
    }

    #[inline]
    fn top_mut(&mut self) -> Option<&mut T> {
        self.last_mut()
    }
}
