use core::{fmt, iter::FusedIterator, mem::MaybeUninit, ptr, slice};

pub struct IntoIter<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    start: usize,
    end: usize,
}

impl<T, const N: usize> IntoIter<T, N> {
    #[inline(always)]
    pub(crate) fn new(data: [MaybeUninit<T>; N], len: usize) -> Self {
        Self {
            data,
            start: 0,
            end: len,
        }
    }
}

impl<T: Clone, const N: usize> Clone for IntoIter<T, N> {
    fn clone(&self) -> Self {
        // Note, we don't really need to match the exact same alive range, so
        // we can just clone into offset 0 regardless of where `self` is.
        let mut new = Self {
            data: [const { MaybeUninit::uninit() }; N],
            start: 0,
            end: 0,
        };

        // Clone all alive elements.
        for (src, dst) in self.as_slice().iter().zip(&mut new.data) {
            // Write a clone into the new array, then update its alive range.
            // If cloning panics, we'll correctly drop the previous items.
            dst.write(src.clone());
            // This addition cannot overflow as we're iterating a slice
            new.end += 1;
        }

        new
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

impl<T, const N: usize> IntoIter<T, N> {
    pub fn as_slice(&self) -> &[T] {
        let ptr = (&raw const self.data) as *const T;
        unsafe { slice::from_raw_parts(ptr.add(self.start), self.len()) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let ptr = (&raw mut self.data) as *mut T;
        unsafe { slice::from_raw_parts_mut(ptr.add(self.start), self.len()) }
    }
}

impl<T, const N: usize> AsRef<[T]> for IntoIter<T, N> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.len() == 0 {
            return None;
        }
        let value = unsafe { self.data.get_unchecked(self.start).assume_init_read() };
        self.start += 1;
        Some(value)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        if self.len() == 0 {
            return None;
        }
        self.end -= 1;
        let value = unsafe { self.data.get_unchecked(self.end).assume_init_read() };
        Some(value)
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

impl<T, const N: usize> Drop for IntoIter<T, N> {
    fn drop(&mut self) {
        // destroy the remaining elements
        unsafe { ptr::drop_in_place(self.as_mut_slice()) };
    }
}
