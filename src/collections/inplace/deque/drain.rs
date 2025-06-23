use core::{
    iter::FusedIterator,
    marker::PhantomData,
    mem,
    ptr::NonNull,
    {fmt, ptr},
};

use super::InplaceDeque;

pub struct Drain<'a, T: 'a, const N: usize> {
    // We can't just use a &mut InplaceDeque<T, N>, as that would make Drain invariant over T
    // and we want it to be covariant instead
    deque: NonNull<InplaceDeque<T, N>>,
    // drain_start is stored in deque.len
    drain_len: usize,
    // index into the logical array, not the physical one (always lies in [0..deque.len))
    index: usize,
    // number of elements remaining after dropping the drain
    new_len: usize,
    remaining: usize,
    // Needed to make Drain covariant over T
    _marker: PhantomData<&'a T>,
}

impl<'a, T, const N: usize> Drain<'a, T, N> {
    pub(super) unsafe fn new(
        deque: &'a mut InplaceDeque<T, N>,
        drain_start: usize,
        drain_len: usize,
    ) -> Self {
        let orig_len = mem::replace(&mut deque.len, drain_start);
        let new_len = orig_len - drain_len;
        Self {
            deque: NonNull::from(deque),
            drain_len,
            index: drain_start,
            new_len,
            remaining: drain_len,
            _marker: PhantomData,
        }
    }

    // Only returns pointers to the slices, as that's all we need
    // to drop them. May only be called if `self.remaining != 0`.
    unsafe fn as_slices(&self) -> (*mut [T], *mut [T]) {
        unsafe {
            let deque = self.deque.as_ref();

            // We know that `self.idx + self.remaining <= deque.len <= usize::MAX`, so this won't overflow.
            let logical_remaining_range = self.index..self.index + self.remaining;

            // SAFETY: `logical_remaining_range` represents the
            // range into the logical buffer of elements that
            // haven't been drained yet, so they're all initialized,
            // and `slice::range(start..end, end) == start..end`,
            // so the preconditions for `slice_ranges` are met.
            let (a_range, b_range) =
                deque.slice_ranges(logical_remaining_range.clone(), logical_remaining_range.end);
            (deque.buffer_range(a_range), deque.buffer_range(b_range))
        }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for Drain<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Drain")
            .field(&self.drain_len)
            .field(&self.index)
            .field(&self.new_len)
            .field(&self.remaining)
            .finish()
    }
}

unsafe impl<T: Sync, const N: usize> Sync for Drain<'_, T, N> {}
unsafe impl<T: Send, const N: usize> Send for Drain<'_, T, N> {}

impl<T, const N: usize> Drop for Drain<'_, T, N> {
    fn drop(&mut self) {
        struct DropGuard<'r, 'a, T, const N: usize>(&'r mut Drain<'a, T, N>);

        let guard = DropGuard(self);

        if mem::needs_drop::<T>() && guard.0.remaining != 0 {
            unsafe {
                // SAFETY: We just checked that `self.remaining != 0`.
                let (front, back) = guard.0.as_slices();
                // since idx is a logical index, we don't need to worry about wrapping.
                guard.0.index += front.len();
                guard.0.remaining -= front.len();
                ptr::drop_in_place(front);
                guard.0.remaining = 0;
                ptr::drop_in_place(back);
            }
        }

        // Dropping `guard` handles moving the remaining elements into place.
        impl<'r, 'a, T, const N: usize> Drop for DropGuard<'r, 'a, T, N> {
            #[inline]
            fn drop(&mut self) {
                if mem::needs_drop::<T>() && self.0.remaining != 0 {
                    unsafe {
                        // SAFETY: We just checked that `self.remaining != 0`.
                        let (front, back) = self.0.as_slices();
                        ptr::drop_in_place(front);
                        ptr::drop_in_place(back);
                    }
                }

                let source_deque = unsafe { self.0.deque.as_mut() };

                let drain_len = self.0.drain_len;
                let new_len = self.0.new_len;

                if mem::size_of::<T>() == 0 {
                    // no need to copy around any memory if T is a ZST
                    source_deque.len = new_len;
                    return;
                }

                let head_len = source_deque.len; // #elements in front of the drain
                let tail_len = new_len - head_len; // #elements behind the drain

                // Next, we will fill the hole left by the drain with as few writes as possible.
                // The code below handles the following control flow and reduces the amount of
                // branches under the assumption that `head_len == 0 || tail_len == 0`, i.e.
                // draining at the front or at the back of the dequeue is especially common.
                //
                // H = "head index" = `deque.head`
                // h = elements in front of the drain
                // d = elements in the drain
                // t = elements behind the drain
                //
                // Note that the buffer may wrap at any point and the wrapping is handled by
                // `wrap_copy` and `to_physical_index`.
                //
                // Case 1: if `head_len == 0 && tail_len == 0`
                // Everything was drained, reset the head index back to 0.
                //             H
                // [ . . . . . d d d d . . . . . ]
                //   H
                // [ . . . . . . . . . . . . . . ]
                //
                // Case 2: else if `tail_len == 0`
                // Don't move data or the head index.
                //         H
                // [ . . . h h h h d d d d . . . ]
                //         H
                // [ . . . h h h h . . . . . . . ]
                //
                // Case 3: else if `head_len == 0`
                // Don't move data, but move the head index.
                //         H
                // [ . . . d d d d t t t t . . . ]
                //                 H
                // [ . . . . . . . t t t t . . . ]
                //
                // Case 4: else if `tail_len <= head_len`
                // Move data, but not the head index.
                //       H
                // [ . . h h h h d d d d t t . . ]
                //       H
                // [ . . h h h h t t . . . . . . ]
                //
                // Case 5: else
                // Move data and the head index.
                //       H
                // [ . . h h d d d d t t t t . . ]
                //               H
                // [ . . . . . . h h t t t t . . ]

                // When draining at the front (`.drain(..n)`) or at the back (`.drain(n..)`),
                // we don't need to copy any data. The number of elements copied would be 0.
                if head_len != 0 && tail_len != 0 {
                    join_head_and_tail_wrapping(source_deque, drain_len, head_len, tail_len);
                    // Marking this function as cold helps LLVM to eliminate it entirely if
                    // this branch is never taken.
                    // We use `#[cold]` instead of `#[inline(never)]`, because inlining this
                    // function into the general case (`.drain(n..m)`) is fine.
                    // See `tests/codegen/InplaceDeque-drain.rs` for a test.
                    #[cold]
                    fn join_head_and_tail_wrapping<T, const N: usize>(
                        source_deque: &mut InplaceDeque<T, N>,
                        drain_len: usize,
                        head_len: usize,
                        tail_len: usize,
                    ) {
                        // Pick whether to move the head or the tail here.
                        let (src, dst, len);
                        if head_len < tail_len {
                            src = source_deque.head;
                            dst = source_deque.to_physical_index(drain_len);
                            len = head_len;
                        } else {
                            src = source_deque.to_physical_index(head_len + drain_len);
                            dst = source_deque.to_physical_index(head_len);
                            len = tail_len;
                        };

                        unsafe {
                            source_deque.wrap_copy(src, dst, len);
                        }
                    }
                }

                if new_len == 0 {
                    // Special case: If the entire dequeue was drained, reset the head back to 0,
                    // like `.clear()` does.
                    source_deque.head = 0;
                } else if head_len < tail_len {
                    // If we moved the head above, then we need to adjust the head index here.
                    source_deque.head = source_deque.to_physical_index(drain_len);
                }
                source_deque.len = new_len;
            }
        }
    }
}

impl<T, const N: usize> Iterator for Drain<'_, T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.remaining == 0 {
            return None;
        }
        let wrapped_index = unsafe { self.deque.as_ref().to_physical_index(self.index) };
        self.index += 1;
        self.remaining -= 1;
        Some(unsafe {
            self.deque
                .as_mut()
                .buf
                .get_unchecked(wrapped_index)
                .assume_init_read()
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.remaining;
        (len, Some(len))
    }
}

impl<T, const N: usize> DoubleEndedIterator for Drain<'_, T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let wrapped_index = unsafe {
            self.deque
                .as_ref()
                .to_physical_index(self.index + self.remaining)
        };
        Some(unsafe {
            self.deque
                .as_mut()
                .buf
                .get_unchecked(wrapped_index)
                .assume_init_read()
        })
    }
}

impl<T, const N: usize> ExactSizeIterator for Drain<'_, T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, const N: usize> FusedIterator for Drain<'_, T, N> {}
