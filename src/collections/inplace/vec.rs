use std::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
    slice::SliceIndex,
};

pub struct InplaceVec<T, const N: usize> {
    buf: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> InplaceVec<T, N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn truncate(&mut self, len: usize) {
        if len > self.len {
            return;
        }
        let len_to_drop = self.len - len;
        let slice_to_drop = unsafe { self.buf.get_unchecked_mut(len..len_to_drop) };
        self.len = len;
        unsafe { std::ptr::drop_in_place(slice_to_drop as *mut _ as *mut [T]) };
    }

    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(&self.buf as *const _ as _, self.len) }
    }

    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(&mut self.buf as *mut _ as _, self.len) }
    }

    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> Result<T, Box<str>> {
        let len = self.len;
        if index >= len {
            let err = format!("swap_remove index (is {index}) should be < len (is {len})");
            return Err(err.into_boxed_str());
        }
        let buf = &mut self.buf;
        let value = unsafe { buf.get_unchecked(index).assume_init_read() };
        let last_value = unsafe { buf.get_unchecked(len - 1).assume_init_read() };
        unsafe { buf.get_unchecked_mut(index) }.write(last_value);
        self.len -= 1;
        Ok(value)
    }

    pub fn insert(&mut self, index: usize, element: T) -> Result<(), Box<str>> {
        if self.is_full() {
            let err = "InplaceVec is full!".to_string();
            return Err(err.into_boxed_str());
        }
        let len = self.len;
        if index > len {
            let err = format!("insertion index (is {index}) should be <= len (is {len})");
            return Err(err.into_boxed_str());
        }
        let buf = &mut self.buf;
        let insert_place = unsafe { buf.get_unchecked_mut(index) };
        if index < len {
            let insert_place_ptr = insert_place as *mut _ as *mut T;
            unsafe { std::ptr::copy(insert_place_ptr, insert_place_ptr.add(1), len - index) };
        }
        insert_place.write(element);
        self.len += 1;
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<T, Box<str>> {
        let len = self.len;
        if index >= len {
            let err = format!("removal index (is {index}) should be < len (is {len})");
            return Err(err.into_boxed_str());
        }
        let buf = &mut self.buf;
        let remove_place = unsafe { buf.get_unchecked_mut(index) };
        let value = unsafe { remove_place.assume_init_read() };
        let remove_place_ptr = remove_place as *mut _ as *mut T;
        unsafe { std::ptr::copy(remove_place_ptr.add(1), remove_place_ptr, len - index - 1) };
        Ok(value)
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.retain_mut(|x| f(x));
    }

    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let original_len = self.len;

        if original_len == 0 {
            // Empty case: explicit return allows better optimization, vs letting compiler infer it
            return;
        }

        // Avoid double drop if the drop guard is not executed,
        // since we may make some holes during the process.
        self.len = 0;

        // Vec: [Kept, Kept, Hole, Hole, Hole, Hole, Unchecked, Unchecked]
        //      |<-              processed len   ->| ^- next to check
        //                  |<-  deleted cnt     ->|
        //      |<-              original_len                          ->|
        // Kept: Elements which predicate returns true on.
        // Hole: Moved or dropped element slot.
        // Unchecked: Unchecked valid elements.
        //
        // This drop guard will be invoked when predicate or `drop` of element panicked.
        // It shifts unchecked elements to cover holes and `set_len` to the correct length.
        // In cases when predicate and `drop` never panick, it will be optimized out.
        struct BackshiftOnDrop<'a, T, const N: usize> {
            v: &'a mut InplaceVec<T, N>,
            processed_len: usize,
            deleted_cnt: usize,
            original_len: usize,
        }

        impl<T, const N: usize> Drop for BackshiftOnDrop<'_, T, N> {
            fn drop(&mut self) {
                if self.deleted_cnt > 0 {
                    // SAFETY: Trailing unchecked items must be valid since we never touch them.
                    unsafe {
                        std::ptr::copy(
                            self.v.as_ptr().add(self.processed_len),
                            self.v
                                .as_mut_ptr()
                                .add(self.processed_len - self.deleted_cnt),
                            self.original_len - self.processed_len,
                        )
                    };
                }
                // SAFETY: After filling holes, all items are in contiguous memory.
                self.v.len = self.original_len - self.deleted_cnt;
            }
        }

        let mut g = BackshiftOnDrop {
            v: self,
            processed_len: 0,
            deleted_cnt: 0,
            original_len,
        };

        fn process_loop<F, T, const N: usize, const DELETED: bool>(
            original_len: usize,
            f: &mut F,
            g: &mut BackshiftOnDrop<'_, T, N>,
        ) where
            F: FnMut(&mut T) -> bool,
        {
            while g.processed_len != original_len {
                // SAFETY: Unchecked element must be valid.
                let cur = unsafe { &mut *g.v.as_mut_ptr().add(g.processed_len) };
                if !f(cur) {
                    // Advance early to avoid double drop if `drop_in_place` panicked.
                    g.processed_len += 1;
                    g.deleted_cnt += 1;
                    // SAFETY: We never touch this element again after dropped.
                    unsafe { std::ptr::drop_in_place(cur) };
                    // We already advanced the counter.
                    if DELETED {
                        continue;
                    } else {
                        break;
                    }
                }
                if DELETED {
                    // SAFETY: `deleted_cnt` > 0, so the hole slot must not overlap with current element.
                    // We use copy for move, and never touch this element again.
                    unsafe {
                        let hole_slot = g.v.as_mut_ptr().add(g.processed_len - g.deleted_cnt);
                        std::ptr::copy_nonoverlapping(cur, hole_slot, 1);
                    }
                }
                g.processed_len += 1;
            }
        }

        // Stage 1: Nothing was deleted.
        process_loop::<F, T, N, false>(original_len, &mut f, &mut g);

        // Stage 2: Some elements were deleted.
        process_loop::<F, T, N, true>(original_len, &mut f, &mut g);

        // All item are processed. This can be optimized to `set_len` by LLVM.
        drop(g);
    }

    pub fn dedup_by_key<F, K>(&mut self, mut key: F)
    where
        F: FnMut(&mut T) -> K,
        K: PartialEq,
    {
        self.dedup_by(|a, b| key(a) == key(b));
    }

    pub fn dedup_by<F>(&mut self, mut same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        let len = self.len;
        if len <= 1 {
            return;
        }

        // Check if we ever want to remove anything.
        // This allows to use copy_non_overlapping in next cycle.
        // And avoids any memory writes if we don't need to remove anything.
        let mut first_duplicate_idx: usize = 1;
        let start = self.as_mut_ptr();
        while first_duplicate_idx != len {
            let found_duplicate = unsafe {
                // SAFETY: first_duplicate always in range [1..len)
                // Note that we start iteration from 1 so we never overflow.
                let prev = start.add(first_duplicate_idx.wrapping_sub(1));
                let current = start.add(first_duplicate_idx);
                // We explicitly say in docs that references are reversed.
                same_bucket(&mut *current, &mut *prev)
            };
            if found_duplicate {
                break;
            }
            first_duplicate_idx += 1;
        }
        // Don't need to remove anything.
        // We cannot get bigger than len.
        if first_duplicate_idx == len {
            return;
        }

        /* INVARIANT: vec.len() > read > write > write-1 >= 0 */
        struct FillGapOnDrop<'a, T, const N: usize> {
            /* Offset of the element we want to check if it is duplicate */
            read: usize,

            /* Offset of the place where we want to place the non-duplicate
             * when we find it. */
            write: usize,

            /* The Vec that would need correction if `same_bucket` panicked */
            vec: &'a mut InplaceVec<T, N>,
        }

        impl<'a, T, const N: usize> Drop for FillGapOnDrop<'a, T, N> {
            fn drop(&mut self) {
                /* This code gets executed when `same_bucket` panics */

                /* SAFETY: invariant guarantees that `read - write`
                 * and `len - read` never overflow and that the copy is always
                 * in-bounds. */
                unsafe {
                    let ptr = self.vec.as_mut_ptr();
                    let len = self.vec.len;

                    /* How many items were left when `same_bucket` panicked.
                     * Basically vec[read..].len() */
                    let items_left = len.wrapping_sub(self.read);

                    /* Pointer to first item in vec[write..write+items_left] slice */
                    let dropped_ptr = ptr.add(self.write);
                    /* Pointer to first item in vec[read..] slice */
                    let valid_ptr = ptr.add(self.read);

                    /* Copy `vec[read..]` to `vec[write..write+items_left]`.
                     * The slices can overlap, so `copy_nonoverlapping` cannot be used */
                    std::ptr::copy(valid_ptr, dropped_ptr, items_left);

                    /* How many items have been already dropped
                     * Basically vec[read..write].len() */
                    let dropped = self.read.wrapping_sub(self.write);

                    self.vec.len = len - dropped;
                }
            }
        }

        /* Drop items while going through Vec, it should be more efficient than
         * doing slice partition_dedup + truncate */

        // Construct gap first and then drop item to avoid memory corruption if `T::drop` panics.
        let mut gap = FillGapOnDrop {
            read: first_duplicate_idx + 1,
            write: first_duplicate_idx,
            vec: self,
        };
        unsafe {
            // SAFETY: we checked that first_duplicate_idx in bounds before.
            // If drop panics, `gap` would remove this item without drop.
            std::ptr::drop_in_place(start.add(first_duplicate_idx));
        }

        /* SAFETY: Because of the invariant, read_ptr, prev_ptr and write_ptr
         * are always in-bounds and read_ptr never aliases prev_ptr */
        unsafe {
            while gap.read < len {
                let read_ptr = start.add(gap.read);
                let prev_ptr = start.add(gap.write.wrapping_sub(1));

                // We explicitly say in docs that references are reversed.
                let found_duplicate = same_bucket(&mut *read_ptr, &mut *prev_ptr);
                if found_duplicate {
                    // Increase `gap.read` now since the drop may panic.
                    gap.read += 1;
                    /* We have found duplicate, drop it in-place */
                    std::ptr::drop_in_place(read_ptr);
                } else {
                    let write_ptr = start.add(gap.write);

                    /* read_ptr cannot be equal to write_ptr because at this point
                     * we guaranteed to skip at least one element (before loop starts).
                     */
                    std::ptr::copy_nonoverlapping(read_ptr, write_ptr, 1);

                    /* We have filled that place, so go further */
                    gap.write += 1;
                    gap.read += 1;
                }
            }

            /* Technically we could let `gap` clean up with its Drop, but
             * when `same_bucket` is guaranteed to not panic, this bloats a little
             * the codegen, so we just do it manually */
            gap.vec.len = gap.write;
            std::mem::forget(gap);
        }
    }

    #[inline]
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        unsafe { self.buf.get_unchecked_mut(self.len) }.write(value);
        self.len += 1;
        Ok(())
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        self.len -= 1;
        unsafe { std::hint::assert_unchecked(self.len < self.capacity()) };
        Some(unsafe { self.buf.get_unchecked(self.len).assume_init_read() })
    }

    pub fn pop_if(&mut self, predicate: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let last = self.last_mut()?;
        if predicate(last) { self.pop() } else { None }
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) -> Result<(), Box<str>> {
        if other.len > self.capacity() - self.len {
            let err = format!(
                "self.len() (is {}) + other.len() (is {}) > self.capacity() (is {})",
                self.len,
                other.len,
                self.capacity()
            );
            return Err(err.into_boxed_str());
        }
        let count = other.len;
        other.len = 0;
        let dst = unsafe { self.as_mut_ptr().add(self.len) };
        unsafe { std::ptr::copy_nonoverlapping(other.as_ptr(), dst, count) };
        self.len += count;
        Ok(())
    }

    #[inline]
    pub fn clear(&mut self) {
        let slice_to_drop = self.as_mut_slice() as *mut _;
        self.len = 0;
        unsafe { std::ptr::drop_in_place(slice_to_drop) };
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == self.capacity()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// # Safety
    ///
    /// - `new_len` must be less than or equal to [`capacity()`].
    /// - The elements at `old_len..new_len` must be initialized.
    ///
    /// [`capacity()`]: Self::capacity
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        debug_assert!(new_len < self.capacity());
        self.len = new_len;
    }

    #[inline]
    pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
        unsafe { self.buf.get_unchecked_mut(self.len..) }
    }
}

impl<T, const N: usize> Drop for InplaceVec<T, N> {
    fn drop(&mut self) {
        let slice_to_drop = std::ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), self.len);
        unsafe { std::ptr::drop_in_place(slice_to_drop) };
    }
}

impl<T, const N: usize> Default for InplaceVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq, const N: usize> InplaceVec<T, N> {
    #[inline]
    pub fn dedup(&mut self) {
        self.dedup_by(|a, b| a == b);
    }
}

impl<T: PartialEq, const N1: usize, const N2: usize> PartialEq<InplaceVec<T, N2>>
    for InplaceVec<T, N1>
{
    #[inline]
    fn eq(&self, other: &InplaceVec<T, N2>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}

impl<T: PartialOrd, const N1: usize, const N2: usize> PartialOrd<InplaceVec<T, N2>>
    for InplaceVec<T, N1>
{
    #[inline]
    fn partial_cmp(&self, other: &InplaceVec<T, N2>) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<T: Eq, const N: usize> Eq for InplaceVec<T, N> {}

impl<T: Ord, const N: usize> Ord for InplaceVec<T, N> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T, const N: usize> std::ops::Deref for InplaceVec<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const N: usize> std::ops::DerefMut for InplaceVec<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T: Clone, const N: usize> Clone for InplaceVec<T, N> {
    fn clone(&self) -> Self {
        let mut v = Self::new();
        let len = self.len;
        unsafe { v.buf.get_unchecked_mut(..len) }
            .iter_mut()
            .zip(self.as_slice())
            .for_each(|(dst, src)| {
                dst.write(src.clone());
            });
        v.len = len;
        v
    }
}

impl<T: std::fmt::Debug, const N: usize> std::fmt::Debug for InplaceVec<T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: std::hash::Hash, const N: usize> std::hash::Hash for InplaceVec<T, N> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&**self, state);
    }
}

impl<T, I: SliceIndex<[T]>, const N: usize> Index<I> for InplaceVec<T, N> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}

impl<T, I: SliceIndex<[T]>, const N: usize> IndexMut<I> for InplaceVec<T, N> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl<T, const N: usize> FromIterator<T> for InplaceVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v = Self::new();
        let len = v
            .buf
            .iter_mut()
            .zip(iter)
            .map(|(dst, src)| dst.write(src))
            .count();
        v.len = len;
        v
    }
}

impl<T, const N: usize> From<[T; N]> for InplaceVec<T, N> {
    fn from(value: [T; N]) -> Self {
        Self {
            buf: value.map(MaybeUninit::new),
            len: N,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1() {
        let mut v = InplaceVec::from([1, 2, 3, 4, 5]);
        assert_eq!(v.push(42), Err(42));
        println!("{:?}", v);
        v.retain(|x| x % 2 == 1);
        println!("{:?}", v);
    }

    #[test]
    fn t2() {
        let v: InplaceVec<_, 5> = (0..10).collect();
        println!("{:?}", v);
    }
}
