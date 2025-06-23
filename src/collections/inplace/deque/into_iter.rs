use core::fmt;
use core::iter::FusedIterator;

use super::InplaceDeque;

#[derive(Clone)]
pub struct IntoIter<T, const N: usize> {
    inner: InplaceDeque<T, N>,
}

impl<T, const N: usize> IntoIter<T, N> {
    pub(super) fn new(inner: InplaceDeque<T, N>) -> Self {
        IntoIter { inner }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.inner).finish()
    }
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.inner.pop_front()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.inner.len();
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.len
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.inner.pop_back()
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        self.inner.pop_back()
    }
}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len
    }
}
