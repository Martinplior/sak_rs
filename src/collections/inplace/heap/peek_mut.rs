use core::{
    fmt,
    num::NonZero,
    ops::{Deref, DerefMut},
};

use super::InplaceHeap;

pub struct PeekMut<'a, T: 'a + Ord, const N: usize> {
    pub(super) heap: &'a mut InplaceHeap<T, N>,
    // If a set_len + sift_down are required, this is Some. If a &mut T has not
    // yet been exposed to peek_mut()'s caller, it's None.
    pub(super) original_len: Option<NonZero<usize>>,
}

impl<T: Ord + fmt::Debug, const N: usize> fmt::Debug for PeekMut<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PeekMut").field(&self.heap.data[0]).finish()
    }
}

impl<T: Ord, const N: usize> Drop for PeekMut<'_, T, N> {
    fn drop(&mut self) {
        if let Some(original_len) = self.original_len {
            // SAFETY: That's how many elements were in the Vec at the time of
            // the PeekMut::deref_mut call, and therefore also at the time of
            // the InplaceHeap::peek_mut call. Since the PeekMut did not end up
            // getting leaked, we are now undoing the leak amplification that
            // the DerefMut prepared for.
            unsafe { self.heap.data.set_len(original_len.get()) };

            // SAFETY: PeekMut is only instantiated for non-empty heaps.
            unsafe { self.heap.sift_down(0) };
        }
    }
}

impl<T: Ord, const N: usize> Deref for PeekMut<'_, T, N> {
    type Target = T;

    fn deref(&self) -> &T {
        debug_assert!(!self.heap.is_empty());
        // SAFE: PeekMut is only instantiated for non-empty heaps
        unsafe { self.heap.data.get_unchecked(0) }
    }
}

impl<T: Ord, const N: usize> DerefMut for PeekMut<'_, T, N> {
    fn deref_mut(&mut self) -> &mut T {
        debug_assert!(!self.heap.is_empty());

        let len = self.heap.len();
        if len > 1 {
            // Here we preemptively leak all the rest of the underlying vector
            // after the currently max element. If the caller mutates the &mut T
            // we're about to give them, and then leaks the PeekMut, all these
            // elements will remain leaked. If they don't leak the PeekMut, then
            // either Drop or PeekMut::pop will un-leak the vector elements.
            //
            // This is technique is described throughout several other places in
            // the standard library as "leak amplification".
            unsafe {
                // SAFETY: len > 1 so len != 0.
                self.original_len = Some(NonZero::new_unchecked(len));
                // SAFETY: len > 1 so all this does for now is leak elements,
                // which is safe.
                self.heap.data.set_len(1);
            }
        }

        // SAFE: PeekMut is only instantiated for non-empty heaps
        unsafe { self.heap.data.get_unchecked_mut(0) }
    }
}

impl<'a, T: Ord, const N: usize> PeekMut<'a, T, N> {
    /// Removes the peeked value from the heap and returns it.
    pub fn pop(mut this: PeekMut<'a, T, N>) -> T {
        if let Some(original_len) = this.original_len.take() {
            // SAFETY: This is how many elements were in the Vec at the time of
            // the InplaceHeap::peek_mut call.
            unsafe { this.heap.data.set_len(original_len.get()) };

            // Unlike in Drop, here we don't also need to do a sift_down even if
            // the caller could've mutated the element. It is removed from the
            // heap on the next line and pop() is not sensitive to its value.
        }

        // SAFETY: Have a `PeekMut` element proves that the associated binary heap being non-empty,
        // so the `pop` operation will not fail.
        unsafe { this.heap.pop().unwrap_unchecked() }
    }
}
