pub mod peek_mut;

mod hole;

use core::{fmt, mem, ptr, slice};

use crate::collections::InplaceVec;

use hole::Hole;
use peek_mut::PeekMut;

pub struct InplaceHeap<T, const N: usize> {
    data: InplaceVec<T, N>,
}

impl<T: Default, const N: usize> Default for InplaceHeap<T, N> {
    #[inline]
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl<T: Ord + Clone, const N: usize> Clone for InplaceHeap<T, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for InplaceHeap<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

struct RebuildOnDrop<'a, T: Ord, const N: usize> {
    heap: &'a mut InplaceHeap<T, N>,
    rebuild_from: usize,
}

impl<T: Ord, const N: usize> Drop for RebuildOnDrop<'_, T, N> {
    fn drop(&mut self) {
        self.heap.rebuild_tail(self.rebuild_from);
    }
}

impl<T: Ord, const N: usize> InplaceHeap<T, N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            data: InplaceVec::new(),
        }
    }

    pub fn peek_mut(&mut self) -> Option<PeekMut<'_, T, N>> {
        if self.is_empty() {
            None
        } else {
            Some(PeekMut {
                heap: self,
                original_len: None,
            })
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        self.data.pop().map(|mut item| {
            if !self.is_empty() {
                mem::swap(&mut item, &mut self.data[0]);
                // SAFETY: !self.is_empty() means that self.len() > 0
                unsafe { self.sift_down_to_bottom(0) };
            }
            item
        })
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        let old_len = self.len();
        self.data.push(item)?;
        // SAFETY: Since we pushed a new item it means that
        //  old_len = self.len() - 1 < self.len()
        unsafe { self.sift_up(0, old_len) };
        Ok(())
    }

    pub fn into_sorted_vec(mut self) -> InplaceVec<T, N> {
        let mut end = self.len();
        while end > 1 {
            end -= 1;
            // SAFETY: `end` goes from `self.len() - 1` to 1 (both included),
            //  so it's always a valid index to access.
            //  It is safe to access index 0 (i.e. `ptr`), because
            //  1 <= end < self.len(), which means self.len() >= 2.
            unsafe {
                let ptr = self.data.as_mut_ptr();
                ptr::swap(ptr, ptr.add(end));
            }
            // SAFETY: `end` goes from `self.len() - 1` to 1 (both included) so:
            //  0 < 1 <= end <= self.len() - 1 < self.len()
            //  Which means 0 < end and end < self.len().
            unsafe { self.sift_down_range(0, end) };
        }
        self.into_vec()
    }

    pub fn append(&mut self, other: &mut Self) -> Result<(), Box<str>> {
        if self.len() < other.len() {
            mem::swap(self, other);
        }
        let start = self.data.len();
        self.data.append(&mut other.data)?;
        self.rebuild_tail(start);
        Ok(())
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        // rebuild_start will be updated to the first touched element below, and the rebuild will
        // only be done for the tail.
        let mut guard = RebuildOnDrop {
            rebuild_from: self.len(),
            heap: self,
        };
        let mut i = 0;

        guard.heap.data.retain(|e| {
            let keep = f(e);
            if !keep && i < guard.rebuild_from {
                guard.rebuild_from = i;
            }
            i += 1;
            keep
        });
    }
}

impl<T, const N: usize> InplaceHeap<T, N> {
    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.data.iter()
    }

    #[inline]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.data.as_slice()
    }

    #[inline]
    pub fn into_vec(self) -> InplaceVec<T, N> {
        self.into()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    pub fn drain(&mut self) -> super::vec::drain::Drain<'_, T, N> {
        self.data.drain(..)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl<T: Ord, const N: usize> InplaceHeap<T, N> {
    /// # Safety
    ///
    /// The caller must guarantee that `pos < self.len()`.
    ///
    /// Returns the new position of the element.
    unsafe fn sift_up(&mut self, start: usize, pos: usize) -> usize {
        // Take out the value at `pos` and create a hole.
        // SAFETY: The caller guarantees that pos < self.len()
        let mut hole = unsafe { Hole::new(&mut self.data, pos) };

        while hole.pos() > start {
            let parent = (hole.pos() - 1) / 2;

            // SAFETY: hole.pos() > start >= 0, which means hole.pos() > 0
            //  and so hole.pos() - 1 can't underflow.
            //  This guarantees that parent < hole.pos() so
            //  it's a valid index and also != hole.pos().
            if hole.element() <= unsafe { hole.get(parent) } {
                break;
            }

            // SAFETY: Same as above
            unsafe { hole.move_to(parent) };
        }

        hole.pos()
    }

    /// Take an element at `pos` and move it down the heap,
    /// while its children are larger.
    ///
    /// Returns the new position of the element.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `pos < end <= self.len()`.
    unsafe fn sift_down_range(&mut self, pos: usize, end: usize) -> usize {
        // SAFETY: The caller guarantees that pos < end <= self.len().
        let mut hole = unsafe { Hole::new(&mut self.data, pos) };
        let mut child = 2 * hole.pos() + 1;

        // Loop invariant: child == 2 * hole.pos() + 1.
        while child <= end.saturating_sub(2) {
            // compare with the greater of the two children
            // SAFETY: child < end - 1 < self.len() and
            //  child + 1 < end <= self.len(), so they're valid indexes.
            //  child == 2 * hole.pos() + 1 != hole.pos() and
            //  child + 1 == 2 * hole.pos() + 2 != hole.pos().
            // FIXME: 2 * hole.pos() + 1 or 2 * hole.pos() + 2 could overflow
            //  if T is a ZST
            child += unsafe { hole.get(child) <= hole.get(child + 1) } as usize;

            // if we are already in order, stop.
            // SAFETY: child is now either the old child or the old child+1
            //  We already proven that both are < self.len() and != hole.pos()
            if hole.element() >= unsafe { hole.get(child) } {
                return hole.pos();
            }

            // SAFETY: same as above.
            unsafe { hole.move_to(child) };
            child = 2 * hole.pos() + 1;
        }

        // SAFETY: && short circuit, which means that in the
        //  second condition it's already true that child == end - 1 < self.len().
        if child == end - 1 && hole.element() < unsafe { hole.get(child) } {
            // SAFETY: child is already proven to be a valid index and
            //  child == 2 * hole.pos() + 1 != hole.pos().
            unsafe { hole.move_to(child) };
        }

        hole.pos()
    }

    /// # Safety
    ///
    /// The caller must guarantee that `pos < self.len()`.
    unsafe fn sift_down(&mut self, pos: usize) -> usize {
        let len = self.len();
        // SAFETY: pos < len is guaranteed by the caller and
        //  obviously len = self.len() <= self.len().
        unsafe { self.sift_down_range(pos, len) }
    }

    /// Take an element at `pos` and move it all the way down the heap,
    /// then sift it up to its position.
    ///
    /// Note: This is faster when the element is known to be large / should
    /// be closer to the bottom.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `pos < self.len()`.
    unsafe fn sift_down_to_bottom(&mut self, mut pos: usize) {
        let end = self.len();
        let start = pos;

        // SAFETY: The caller guarantees that pos < self.len().
        let mut hole = unsafe { Hole::new(&mut self.data, pos) };
        let mut child = 2 * hole.pos() + 1;

        // Loop invariant: child == 2 * hole.pos() + 1.
        while child <= end.saturating_sub(2) {
            // SAFETY: child < end - 1 < self.len() and
            //  child + 1 < end <= self.len(), so they're valid indexes.
            //  child == 2 * hole.pos() + 1 != hole.pos() and
            //  child + 1 == 2 * hole.pos() + 2 != hole.pos().
            // FIXME: 2 * hole.pos() + 1 or 2 * hole.pos() + 2 could overflow
            //  if T is a ZST
            child += unsafe { hole.get(child) <= hole.get(child + 1) } as usize;

            // SAFETY: Same as above
            unsafe { hole.move_to(child) };
            child = 2 * hole.pos() + 1;
        }

        if child == end - 1 {
            // SAFETY: child == end - 1 < self.len(), so it's a valid index
            //  and child == 2 * hole.pos() + 1 != hole.pos().
            unsafe { hole.move_to(child) };
        }
        pos = hole.pos();
        drop(hole);

        // SAFETY: pos is the position in the hole and was already proven
        //  to be a valid index.
        unsafe { self.sift_up(start, pos) };
    }

    /// Rebuild assuming data[0..start] is still a proper heap.
    fn rebuild_tail(&mut self, start: usize) {
        if start == self.len() {
            return;
        }

        let tail_len = self.len() - start;

        #[inline(always)]
        fn log2_fast(x: usize) -> usize {
            (usize::BITS - x.leading_zeros() - 1) as usize
        }

        // `rebuild` takes O(self.len()) operations
        // and about 2 * self.len() comparisons in the worst case
        // while repeating `sift_up` takes O(tail_len * log(start)) operations
        // and about 1 * tail_len * log_2(start) comparisons in the worst case,
        // assuming start >= tail_len. For larger heaps, the crossover point
        // no longer follows this reasoning and was determined empirically.
        let better_to_rebuild = if start < tail_len {
            true
        } else if self.len() <= 2048 {
            2 * self.len() < tail_len * log2_fast(start)
        } else {
            2 * self.len() < tail_len * 11
        };

        if better_to_rebuild {
            self.rebuild();
        } else {
            for i in start..self.len() {
                // SAFETY: The index `i` is always less than self.len().
                unsafe { self.sift_up(0, i) };
            }
        }
    }

    fn rebuild(&mut self) {
        let mut n = self.len() / 2;
        while n > 0 {
            n -= 1;
            // SAFETY: n starts from self.len() / 2 and goes down to 0.
            //  The only case when !(n < self.len()) is if
            //  self.len() == 0, but it's ruled out by the loop condition.
            unsafe { self.sift_down(n) };
        }
    }
}

impl<T: Ord, const N: usize> From<InplaceVec<T, N>> for InplaceHeap<T, N> {
    fn from(value: InplaceVec<T, N>) -> Self {
        let mut h = Self { data: value };
        h.rebuild();
        h
    }
}

impl<T: Ord, const N: usize> From<[T; N]> for InplaceHeap<T, N> {
    #[inline]
    fn from(value: [T; N]) -> Self {
        Self::from(InplaceVec::from(value))
    }
}

impl<T, const N: usize> From<InplaceHeap<T, N>> for InplaceVec<T, N> {
    #[inline]
    fn from(value: InplaceHeap<T, N>) -> Self {
        value.data
    }
}

impl<T: Ord, const N: usize> FromIterator<T> for InplaceHeap<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from(InplaceVec::from_iter(iter))
    }
}

impl<T, const N: usize> IntoIterator for InplaceHeap<T, N> {
    type Item = T;
    type IntoIter = super::vec::into_iter::IntoIter<T, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a InplaceHeap<T, N> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
