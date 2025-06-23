pub mod drain;
pub mod into_iter;
pub mod iter;
pub mod iter_mut;

use core::{
    cmp::{self, Ordering},
    fmt, hash, hint,
    mem::{self, MaybeUninit},
    ops::{self, Range, RangeBounds},
    ptr, slice,
};

use drain::Drain;
use into_iter::IntoIter;
use iter::Iter;
use iter_mut::IterMut;

use crate::collections::InplaceVec;

pub struct InplaceDeque<T, const N: usize> {
    buf: [MaybeUninit<T>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> InplaceDeque<T, N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: [const { MaybeUninit::uninit() }; N],
            head: 0,
            len: 0,
        }
    }

    /// `Self::new()` with "power of 2" capacity assertion.
    ///
    /// # See also
    ///
    /// [`assert_pow2_capacity`](Self::assert_pow2_capacity)
    #[inline]
    pub const fn with_pow2_capacity() -> Self
    where
        IsPow2Usize<N>: crate::assert::True,
    {
        Self::new()
    }

    /// assert `self.capacity()` is power of 2 in compile time.
    #[inline(always)]
    pub const fn assert_pow2_capacity(&self)
    where
        IsPow2Usize<N>: crate::assert::True,
    {
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let index = self.to_physical_index(index);
        Some(unsafe { self.buf.get_unchecked(index).assume_init_ref() })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        let index = self.to_physical_index(index);
        Some(unsafe { self.buf.get_unchecked_mut(index).assume_init_mut() })
    }

    pub fn swap(&mut self, i: usize, j: usize) -> Result<(), Box<str>> {
        let len = self.len;
        if i >= len {
            let err = format!("swap i (is {i}) should be < len (is {len}");
            return Err(err.into_boxed_str());
        }
        if j >= len {
            let err = format!("swap j (is {j}) should be < len (is {len}");
            return Err(err.into_boxed_str());
        }
        let index_i = self.to_physical_index(i);
        let index_j = self.to_physical_index(j);
        let ptr = (&mut self.buf) as *mut _ as *mut T;
        unsafe { ptr::swap(ptr.add(index_i), ptr.add(index_j)) };
        Ok(())
    }

    pub fn truncate(&mut self, len: usize) {
        /// Runs the destructor for all items in the slice when it gets dropped (normally or
        /// during unwinding).
        struct Dropper<'a, T>(&'a mut [T]);
        impl<'a, T> Drop for Dropper<'a, T> {
            fn drop(&mut self) {
                unsafe { ptr::drop_in_place(self.0) };
            }
        }

        // Safe because:
        //
        // * Any slice passed to `drop_in_place` is valid; the second case has
        //   `len <= front.len()` and returning on `len > self.len()` ensures
        //   `begin <= back.len()` in the first case
        // * The head of the VecDeque is moved before calling `drop_in_place`,
        //   so no value is dropped twice if `drop_in_place` panics
        unsafe {
            if len >= self.len {
                return;
            }

            let (front, back) = self.as_mut_slices();
            if len > front.len() {
                let begin = len - front.len();
                let drop_back = back.get_unchecked_mut(begin..) as *mut _;
                self.len = len;
                ptr::drop_in_place(drop_back);
            } else {
                let drop_back = back as *mut _;
                let drop_front = front.get_unchecked_mut(len..) as *mut _;
                self.len = len;

                // Make sure the second half is dropped even when a destructor
                // in the first one panics.
                let _back_dropper = Dropper(&mut *drop_back);
                ptr::drop_in_place(drop_front);
            }
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        let (a, b) = self.as_slices();
        Iter::new(a.iter(), b.iter())
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let (a, b) = self.as_mut_slices();
        IterMut::new(a.iter_mut(), b.iter_mut())
    }

    #[inline]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        let (a_range, b_range) = self.slice_ranges(.., self.len);
        // SAFETY: `slice_ranges` always returns valid ranges into
        // the physical buffer.
        unsafe { (&*self.buffer_range(a_range), &*self.buffer_range(b_range)) }
    }

    #[inline]
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        let (a_range, b_range) = self.slice_ranges(.., self.len);
        // SAFETY: `slice_ranges` always returns valid ranges into
        // the physical buffer.
        let a = unsafe { &mut *self.buffer_range(a_range) };
        let b = unsafe { &mut *self.buffer_range(b_range) };
        (a, b)
    }

    #[inline]
    pub fn range<R>(&self, range: R) -> Iter<'_, T>
    where
        R: RangeBounds<usize>,
    {
        let (a_range, b_range) = self.slice_ranges(range, self.len);
        // SAFETY: The ranges returned by `slice_ranges`
        // are valid ranges into the physical buffer, so
        // it's ok to pass them to `buffer_range` and
        // dereference the result.
        let a = unsafe { &*self.buffer_range(a_range) };
        let b = unsafe { &*self.buffer_range(b_range) };
        Iter::new(a.iter(), b.iter())
    }

    #[inline]
    pub fn range_mut<R>(&mut self, range: R) -> IterMut<'_, T>
    where
        R: RangeBounds<usize>,
    {
        let (a_range, b_range) = self.slice_ranges(range, self.len);
        // SAFETY: The ranges returned by `slice_ranges`
        // are valid ranges into the physical buffer, so
        // it's ok to pass them to `buffer_range` and
        // dereference the result.
        let a = unsafe { &mut *self.buffer_range(a_range) };
        let b = unsafe { &mut *self.buffer_range(b_range) };
        IterMut::new(a.iter_mut(), b.iter_mut())
    }

    #[inline]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T, N>
    where
        R: RangeBounds<usize>,
    {
        // Memory safety
        //
        // When the Drain is first created, the source deque is shortened to
        // make sure no uninitialized or moved-from elements are accessible at
        // all if the Drain's destructor never gets to run.
        //
        // Drain will ptr::read out the values to remove.
        // When finished, the remaining data will be copied back to cover the hole,
        // and the head/tail values will be restored correctly.
        //
        let Range { start, end } =
            unsafe { crate::slice::range(range, ..self.len).unwrap_unchecked() };
        let drain_start = start;
        let drain_len = end - start;

        // The deque's elements are parted into three segments:
        // * 0  -> drain_start
        // * drain_start -> drain_start+drain_len
        // * drain_start+drain_len -> self.len
        //
        // H = self.head; T = self.head+self.len; t = drain_start+drain_len; h = drain_head
        //
        // We store drain_start as self.len, and drain_len and self.len as
        // drain_len and orig_len respectively on the Drain. This also
        // truncates the effective array such that if the Drain is leaked, we
        // have forgotten about the potentially moved values after the start of
        // the drain.
        //
        //        H   h   t   T
        // [. . . o o x x o o . . .]
        //
        // "forget" about the values after the start of the drain until after
        // the drain is complete and the Drain destructor is run.

        unsafe { Drain::new(self, drain_start, drain_len) }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
        // Not strictly necessary, but leaves things in a more consistent/predictable state.
        self.head = 0;
    }

    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq<T>,
    {
        let (a, b) = self.as_slices();
        a.contains(x) || b.contains(x)
    }

    #[inline]
    pub fn front(&self) -> Option<&T> {
        self.get(0)
    }

    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    #[inline]
    pub fn back(&self) -> Option<&T> {
        self.get(self.len.wrapping_sub(1))
    }

    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.get_mut(self.len.wrapping_sub(1))
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let old_head = self.head;
        self.head = self.to_physical_index(1);
        self.len -= 1;
        unsafe {
            hint::assert_unchecked(self.len < self.capacity());
            Some(self.buf.get_unchecked(old_head).assume_init_read())
        }
    }

    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        self.len -= 1;
        let old_tail = self.to_physical_index(self.len);
        unsafe {
            hint::assert_unchecked(self.len < self.capacity());
            Some(self.buf.get_unchecked(old_tail).assume_init_read())
        }
    }

    pub fn pop_front_if(&mut self, predicate: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let first = self.front_mut()?;
        if predicate(first) {
            self.pop_front()
        } else {
            None
        }
    }

    pub fn pop_back_if(&mut self, predicate: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let last = self.back_mut()?;
        if predicate(last) {
            self.pop_back()
        } else {
            None
        }
    }

    pub fn push_front(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        self.head = self.wrap_sub(self.head, 1);
        unsafe { self.buf.get_unchecked_mut(self.head).write(value) };
        self.len += 1;
        Ok(())
    }

    pub fn push_back(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        let index = self.to_physical_index(self.len);
        unsafe { self.buf.get_unchecked_mut(index).write(value) };
        self.len += 1;
        Ok(())
    }

    /// returns `None` if `index` is out of bounds.
    pub fn swap_remove_front(&mut self, index: usize) -> Option<T> {
        let len = self.len;
        if index >= len {
            return None;
        }
        if index != 0 {
            unsafe { self.swap(index, 0).unwrap_unchecked() };
        }
        self.pop_front()
    }

    /// returns `None` if `index` is out of bounds.
    pub fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        let len = self.len;
        if index >= len {
            return None;
        }
        if index != len - 1 {
            unsafe { self.swap(index, len - 1).unwrap_unchecked() };
        }
        self.pop_back()
    }

    pub fn insert(&mut self, index: usize, value: T) -> Result<(), (T, Box<str>)> {
        if index > self.len {
            let err = "index out of bounds".to_string();
            return Err((value, err.into_boxed_str()));
        }
        if self.is_full() {
            let err = "InplaceDeque is full!".to_string();
            return Err((value, err.into_boxed_str()));
        }

        let front_len = index;
        let back_len = self.len - index;
        if back_len < front_len {
            // `index + 1` can't overflow, because if index was usize::MAX, then either the
            // assert would've failed, or the deque would've tried to grow past usize::MAX
            // and panicked.
            unsafe {
                // see `remove()` for explanation why this wrap_copy() call is safe.
                self.wrap_copy(
                    self.to_physical_index(index),
                    self.to_physical_index(index + 1),
                    back_len,
                )
            };
        } else {
            let old_head = self.head;
            self.head = self.wrap_sub(self.head, 1);
            unsafe { self.wrap_copy(old_head, self.head, index) };
        }
        // insert value
        let index = self.to_physical_index(index);
        unsafe { self.buf.get_unchecked_mut(index).write(value) };
        self.len += 1;
        Ok(())
    }

    /// returns `None` if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        let wrapped_index = self.to_physical_index(index);
        let value = unsafe { self.buf.get_unchecked(wrapped_index).assume_init_read() };

        let front_len = self.len;
        let back_len = self.len - index - 1;
        // Safety: due to the nature of the if-condition, whichever wrap_copy gets called,
        // its length argument will be at most `self.len / 2`, so there can't be more than
        // one overlapping area.
        if back_len < front_len {
            unsafe { self.wrap_copy(self.wrap_add(wrapped_index, 1), wrapped_index, back_len) };
        } else {
            let old_head = self.head;
            self.head = self.to_physical_index(1);
            unsafe { self.wrap_copy(old_head, self.head, index) };
        }
        self.len -= 1;

        Some(value)
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

        if mem::size_of::<T>() != 0 {
            let (left, right) = other.as_slices();
            unsafe {
                self.copy_slice(self.to_physical_index(self.len), left);
                // no overflow, because self.capacity() >= old_cap + left.len() >= self.len + left.len()
                self.copy_slice(self.to_physical_index(self.len + left.len()), right);
            }
        }
        // SAFETY: Update pointers after copying to avoid leaving doppelganger
        // in case of panics.
        self.len += other.len;
        // Now that we own its values, forget everything in `other`.
        other.len = 0;
        other.head = 0;

        Ok(())
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.retain_mut(|elem| f(elem));
    }

    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let len = self.len;
        let mut index = 0;
        let mut current_index = 0;

        // Stage 1: All values are retained.
        while current_index < len {
            if !f(unsafe { self.get_mut(current_index).unwrap_unchecked() }) {
                current_index += 1;
                break;
            }
            current_index += 1;
            index += 1;
        }
        // Stage 2: Swap retained value into current idx.
        while current_index < len {
            if !f(unsafe { self.get_mut(current_index).unwrap_unchecked() }) {
                current_index += 1;
                continue;
            }
            unsafe { self.swap(index, current_index).unwrap_unchecked() };
            current_index += 1;
            index += 1;
        }
        // Stage 3: Truncate all values after idx.
        if current_index != index {
            self.truncate(index);
        }
    }

    pub fn make_contiguous(&mut self) -> &mut [T] {
        if mem::size_of::<T>() == 0 {
            self.head = 0;
        }
        let ptr = (&raw mut self.buf) as *mut T;
        if self.is_contiguous() {
            return unsafe { slice::from_raw_parts_mut(ptr.add(self.head), self.len) };
        }
        let &mut Self { head, len, .. } = self;
        let capacity = self.capacity();

        let free = capacity - len;
        let head_len = capacity - head;
        let tail = len - head_len;
        let tail_len = tail;

        if free >= head_len {
            // there is enough free space to copy the head in one go,
            // this means that we first shift the tail backwards, and then
            // copy the head to the correct position.
            //
            // from: DEFGH....ABC
            // to:   ABCDEFGH....
            unsafe {
                self.copy(0, head_len, tail_len);
                // ...DEFGH.ABC
                self.copy_nonoverlapping(head, 0, head_len);
                // ABCDEFGH....
            }

            self.head = 0;
        } else if free >= tail_len {
            // there is enough free space to copy the tail in one go,
            // this means that we first shift the head forwards, and then
            // copy the tail to the correct position.
            //
            // from: FGH....ABCDE
            // to:   ...ABCDEFGH.
            unsafe {
                self.copy(head, tail, head_len);
                // FGHABCDE....
                self.copy_nonoverlapping(0, tail + head_len, tail_len);
                // ...ABCDEFGH.
            }

            self.head = tail;
        } else {
            // `free` is smaller than both `head_len` and `tail_len`.
            // the general algorithm for this first moves the slices
            // right next to each other and then uses `slice::rotate`
            // to rotate them into place:
            //
            // initially:   HIJK..ABCDEFG
            // step 1:      ..HIJKABCDEFG
            // step 2:      ..ABCDEFGHIJK
            //
            // or:
            //
            // initially:   FGHIJK..ABCDE
            // step 1:      FGHIJKABCDE..
            // step 2:      ABCDEFGHIJK..

            // pick the shorter of the 2 slices to reduce the amount
            // of memory that needs to be moved around.
            if head_len > tail_len {
                // tail is shorter, so:
                //  1. copy tail forwards
                //  2. rotate used part of the buffer
                //  3. update head to point to the new beginning (which is just `free`)

                unsafe {
                    // if there is no free space in the buffer, then the slices are already
                    // right next to each other and we don't need to move any memory.
                    if free != 0 {
                        // because we only move the tail forward as much as there's free space
                        // behind it, we don't overwrite any elements of the head slice, and
                        // the slices end up right next to each other.
                        self.copy(0, free, tail_len);
                    }

                    // We just copied the tail right next to the head slice,
                    // so all of the elements in the range are initialized
                    let slice = &mut *self.buffer_range(free..self.capacity());

                    // because the deque wasn't contiguous, we know that `tail_len < self.len == slice.len()`,
                    // so this will never panic.
                    slice.rotate_left(tail_len);

                    // the used part of the buffer now is `free..self.capacity()`, so set
                    // `head` to the beginning of that range.
                    self.head = free;
                }
            } else {
                // head is shorter so:
                //  1. copy head backwards
                //  2. rotate used part of the buffer
                //  3. update head to point to the new beginning (which is the beginning of the buffer)

                unsafe {
                    // if there is no free space in the buffer, then the slices are already
                    // right next to each other and we don't need to move any memory.
                    if free != 0 {
                        // copy the head slice to lie right behind the tail slice.
                        self.copy(self.head, tail_len, head_len);
                    }

                    // because we copied the head slice so that both slices lie right
                    // next to each other, all the elements in the range are initialized.
                    let slice = &mut *self.buffer_range(0..self.len);

                    // because the deque wasn't contiguous, we know that `head_len < self.len == slice.len()`
                    // so this will never panic.
                    slice.rotate_right(head_len);

                    // the used part of the buffer now is `0..self.len`, so set
                    // `head` to the beginning of that range.
                    self.head = 0;
                }
            }
        }

        unsafe { slice::from_raw_parts_mut(ptr.add(self.head), self.len) }
    }

    pub fn rotate_left(&mut self, n: usize) -> Result<(), Box<str>> {
        if n > self.len {
            let err = format!("n (is {n}) is out of bounds.");
            return Err(err.into_boxed_str());
        }
        let front_len = n;
        let back_len = self.len - n;
        if front_len <= back_len {
            unsafe { self.rotate_left_inner(n) };
        } else {
            unsafe { self.rotate_right_inner(back_len) };
        }
        Ok(())
    }

    pub fn rotate_right(&mut self, n: usize) -> Result<(), Box<str>> {
        if n > self.len {
            let err = format!("n (is {n}) is out of bounds.");
            return Err(err.into_boxed_str());
        }
        let front_len = n;
        let back_len = self.len - n;
        if front_len <= back_len {
            unsafe { self.rotate_right_inner(n) };
        } else {
            unsafe { self.rotate_left_inner(back_len) };
        }
        Ok(())
    }

    /// If the value is found then [`Result::Ok`] is returned, containing the
    /// index of the matching element. If there are multiple matches, then any
    /// one of the matches could be returned. If the value is not found then
    /// [`Result::Err`] is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search_by`](Self::binary_search_by),
    /// [`binary_search_by_key`](Self::binary_search_by_key), and
    /// [`partition_point`](Self::partition_point).
    ///
    #[inline]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search_by(|e| e.cmp(x))
    }

    /// If the value is found then [`Result::Ok`] is returned, containing the
    /// index of the matching element. If there are multiple matches, then any
    /// one of the matches could be returned. If the value is not found then
    /// [`Result::Err`] is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search`](Self::binary_search),
    /// [`binary_search_by_key`](Self::binary_search_by_key), and
    /// [`partition_point`](Self::partition_point).
    ///
    pub fn binary_search_by<'a, F>(&'a self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        let (front, back) = self.as_slices();
        let cmp_back = back.first().map(&mut f);
        match cmp_back {
            Some(Ordering::Equal) => Ok(front.len()),
            Some(Ordering::Less) => back
                .binary_search_by(f)
                .map(|idx| idx + front.len())
                .map_err(|idx| idx + front.len()),
            _ => front.binary_search_by(f),
        }
    }

    /// If the value is found then [`Result::Ok`] is returned, containing the
    /// index of the matching element. If there are multiple matches, then any
    /// one of the matches could be returned. If the value is not found then
    /// [`Result::Err`] is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search`](Self::binary_search),
    /// [`binary_search_by`](Self::binary_search_by), and
    /// [`partition_point`](Self::partition_point).
    ///
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.binary_search_by(|k| f(k).cmp(b))
    }

    pub fn partition_point<P>(&self, mut pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        let (front, back) = self.as_slices();

        if let Some(true) = back.first().map(&mut pred) {
            back.partition_point(pred) + front.len()
        } else {
            front.partition_point(pred)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == N
    }
}

impl<T, const N: usize> InplaceDeque<T, N> {
    // SAFETY: the following two methods require that the rotation amount
    // be less than half the length of the deque.
    //
    // `wrap_copy` requires that `min(x, capacity() - x) + copy_len <= capacity()`,
    // but then `min` is never more than half the capacity, regardless of x,
    // so it's sound to call here because we're calling with something
    // less than half the length, which is never above half the capacity.

    unsafe fn rotate_left_inner(&mut self, mid: usize) {
        debug_assert!(mid * 2 <= self.len());
        unsafe { self.wrap_copy(self.head, self.to_physical_index(self.len), mid) };
        self.head = self.to_physical_index(mid);
    }

    unsafe fn rotate_right_inner(&mut self, k: usize) {
        debug_assert!(k * 2 <= self.len());
        self.head = self.wrap_sub(self.head, k);
        unsafe { self.wrap_copy(self.to_physical_index(self.len), self.head, k) };
    }

    #[inline]
    fn to_physical_index(&self, index: usize) -> usize {
        self.wrap_add(self.head, index)
    }

    #[inline]
    fn wrap_add(&self, index: usize, addend: usize) -> usize {
        wrap_index::<N>(index.wrapping_add(addend))
    }

    #[inline]
    fn wrap_sub(&self, index: usize, subtrahend: usize) -> usize {
        wrap_index::<N>(index.wrapping_sub(subtrahend).wrapping_add(self.capacity()))
    }

    #[inline]
    unsafe fn buffer_range(&self, range: Range<usize>) -> *mut [T] {
        let ptr = &raw const self.buf as *mut T;
        unsafe { ptr::slice_from_raw_parts_mut(ptr.add(range.start), range.end - range.start) }
    }

    fn slice_ranges<R>(&self, range: R, len: usize) -> (Range<usize>, Range<usize>)
    where
        R: ops::RangeBounds<usize>,
    {
        let Range { start, end } = unsafe { crate::slice::range(range, ..len).unwrap_unchecked() };
        let len = end - start;

        if len == 0 {
            (0..0, 0..0)
        } else {
            // `slice::range` guarantees that `start <= end <= len`.
            // because `len != 0`, we know that `start < end`, so `start < len`
            // and the indexing is valid.
            let wrapped_start = self.to_physical_index(start);

            // this subtraction can never overflow because `wrapped_start` is
            // at most `self.capacity()` (and if `self.capacity != 0`, then `wrapped_start` is strictly less
            // than `self.capacity`).
            let head_len = self.capacity() - wrapped_start;

            if head_len >= len {
                // we know that `len + wrapped_start <= self.capacity <= usize::MAX`, so this addition can't overflow
                (wrapped_start..wrapped_start + len, 0..0)
            } else {
                // can't overflow because of the if condition
                let tail_len = len - head_len;
                (wrapped_start..self.capacity(), 0..tail_len)
            }
        }
    }

    #[inline]
    fn is_contiguous(&self) -> bool {
        self.head <= self.capacity() - self.len
    }

    /// Copies all values from `src` to `dst`, wrapping around if needed.
    /// Assumes capacity is sufficient.
    #[inline]
    unsafe fn copy_slice(&mut self, dst: usize, src: &[T]) {
        debug_assert!(src.len() <= self.capacity());
        let head_room = self.capacity() - dst;
        let ptr = (&raw mut self.buf) as *mut T;
        if src.len() <= head_room {
            unsafe {
                ptr::copy_nonoverlapping(src.as_ptr(), ptr.add(dst), src.len());
            }
        } else {
            let (left, right) = src.split_at(head_room);
            unsafe {
                ptr::copy_nonoverlapping(left.as_ptr(), ptr.add(dst), left.len());
                ptr::copy_nonoverlapping(right.as_ptr(), ptr, right.len());
            }
        }
    }

    /// Copies a contiguous block of memory len long from src to dst
    #[inline]
    unsafe fn copy(&mut self, src: usize, dst: usize, len: usize) {
        debug_assert!(
            dst + len <= self.capacity(),
            "cpy dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            self.capacity()
        );
        debug_assert!(
            src + len <= self.capacity(),
            "cpy dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            self.capacity()
        );
        let ptr = (&raw mut self.buf) as *mut T;
        unsafe { ptr::copy(ptr.add(src), ptr.add(dst), len) };
    }

    /// Copies a contiguous block of memory len long from src to dst
    #[inline]
    unsafe fn copy_nonoverlapping(&mut self, src: usize, dst: usize, len: usize) {
        debug_assert!(
            dst + len <= self.capacity(),
            "cno dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            self.capacity()
        );
        debug_assert!(
            src + len <= self.capacity(),
            "cno dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            self.capacity()
        );
        let ptr = (&raw mut self.buf) as *mut T;
        unsafe { ptr::copy_nonoverlapping(ptr.add(src), ptr.add(dst), len) };
    }

    /// Copies a potentially wrapping block of memory len long from src to dest.
    /// (abs(dst - src) + len) must be no larger than capacity() (There must be at
    /// most one continuous overlapping region between src and dest).
    unsafe fn wrap_copy(&mut self, src: usize, dst: usize, len: usize) {
        debug_assert!(
            cmp::min(src.abs_diff(dst), self.capacity() - src.abs_diff(dst)) + len
                <= self.capacity(),
            "wrc dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            self.capacity()
        );

        // If T is a ZST, don't do any copying.
        if mem::size_of::<T>() == 0 || src == dst || len == 0 {
            return;
        }

        let dst_after_src = self.wrap_sub(dst, src) < len;

        let src_pre_wrap_len = self.capacity() - src;
        let dst_pre_wrap_len = self.capacity() - dst;
        let src_wraps = src_pre_wrap_len < len;
        let dst_wraps = dst_pre_wrap_len < len;

        match (dst_after_src, src_wraps, dst_wraps) {
            (_, false, false) => {
                // src doesn't wrap, dst doesn't wrap
                //
                //        S . . .
                // 1 [_ _ A A B B C C _]
                // 2 [_ _ A A A A B B _]
                //            D . . .
                //
                unsafe {
                    self.copy(src, dst, len);
                }
            }
            (false, false, true) => {
                // dst before src, src doesn't wrap, dst wraps
                //
                //    S . . .
                // 1 [A A B B _ _ _ C C]
                // 2 [A A B B _ _ _ A A]
                // 3 [B B B B _ _ _ A A]
                //    . .           D .
                //
                unsafe {
                    self.copy(src, dst, dst_pre_wrap_len);
                    self.copy(src + dst_pre_wrap_len, 0, len - dst_pre_wrap_len);
                }
            }
            (true, false, true) => {
                // src before dst, src doesn't wrap, dst wraps
                //
                //              S . . .
                // 1 [C C _ _ _ A A B B]
                // 2 [B B _ _ _ A A B B]
                // 3 [B B _ _ _ A A A A]
                //    . .           D .
                //
                unsafe {
                    self.copy(src + dst_pre_wrap_len, 0, len - dst_pre_wrap_len);
                    self.copy(src, dst, dst_pre_wrap_len);
                }
            }
            (false, true, false) => {
                // dst before src, src wraps, dst doesn't wrap
                //
                //    . .           S .
                // 1 [C C _ _ _ A A B B]
                // 2 [C C _ _ _ B B B B]
                // 3 [C C _ _ _ B B C C]
                //              D . . .
                //
                unsafe {
                    self.copy(src, dst, src_pre_wrap_len);
                    self.copy(0, dst + src_pre_wrap_len, len - src_pre_wrap_len);
                }
            }
            (true, true, false) => {
                // src before dst, src wraps, dst doesn't wrap
                //
                //    . .           S .
                // 1 [A A B B _ _ _ C C]
                // 2 [A A A A _ _ _ C C]
                // 3 [C C A A _ _ _ C C]
                //    D . . .
                //
                unsafe {
                    self.copy(0, dst + src_pre_wrap_len, len - src_pre_wrap_len);
                    self.copy(src, dst, src_pre_wrap_len);
                }
            }
            (false, true, true) => {
                // dst before src, src wraps, dst wraps
                //
                //    . . .         S .
                // 1 [A B C D _ E F G H]
                // 2 [A B C D _ E G H H]
                // 3 [A B C D _ E G H A]
                // 4 [B C C D _ E G H A]
                //    . .         D . .
                //
                debug_assert!(dst_pre_wrap_len > src_pre_wrap_len);
                let delta = dst_pre_wrap_len - src_pre_wrap_len;
                unsafe {
                    self.copy(src, dst, src_pre_wrap_len);
                    self.copy(0, dst + src_pre_wrap_len, delta);
                    self.copy(delta, 0, len - dst_pre_wrap_len);
                }
            }
            (true, true, true) => {
                // src before dst, src wraps, dst wraps
                //
                //    . .         S . .
                // 1 [A B C D _ E F G H]
                // 2 [A A B D _ E F G H]
                // 3 [H A B D _ E F G H]
                // 4 [H A B D _ E F F G]
                //    . . .         D .
                //
                debug_assert!(src_pre_wrap_len > dst_pre_wrap_len);
                let delta = src_pre_wrap_len - dst_pre_wrap_len;
                unsafe {
                    self.copy(0, delta, len - src_pre_wrap_len);
                    self.copy(self.capacity() - delta, 0, delta);
                    self.copy(src, dst, dst_pre_wrap_len);
                }
            }
        }
    }
}

impl<T, const N: usize> Default for InplaceDeque<T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, const N: usize> Clone for InplaceDeque<T, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self::from_iter(self.iter().cloned())
    }
}

impl<T, const N: usize> Drop for InplaceDeque<T, N> {
    fn drop(&mut self) {
        struct Dropper<'a, T>(&'a mut [T]);
        impl<'a, T> Drop for Dropper<'a, T> {
            fn drop(&mut self) {
                unsafe { ptr::drop_in_place(self.0) };
            }
        }

        let (front, back) = self.as_mut_slices();
        let _back_dropper = Dropper(back);
        unsafe { ptr::drop_in_place(front) };
    }
}

impl<T: PartialEq, const N: usize> PartialEq for InplaceDeque<T, N> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        let (self_a, self_b) = self.as_slices();
        let (other_a, other_b) = other.as_slices();
        if self_a.len() == other_a.len() {
            self_a == other_a && self_b == other_b
        } else if self_a.len() < other_a.len() {
            // Always divisible in three sections, for example:
            // self:  [a b c|d e f]
            // other: [0 1 2 3|4 5]
            // front = 3, mid = 1,
            // [a b c] == [0 1 2] && [d] == [3] && [e f] == [4 5]
            let front = self_a.len();
            let mid = other_a.len() - front;

            let (other_a_front, other_a_mid) = other_a.split_at(front);
            let (self_b_mid, self_b_back) = self_b.split_at(mid);
            debug_assert_eq!(self_a.len(), other_a_front.len());
            debug_assert_eq!(self_b_mid.len(), other_a_mid.len());
            debug_assert_eq!(self_b_back.len(), other_b.len());
            self_a == other_a_front && self_b_mid == other_a_mid && self_b_back == other_b
        } else {
            let front = other_a.len();
            let mid = self_a.len() - front;

            let (self_a_front, self_a_mid) = self_a.split_at(front);
            let (other_b_mid, other_b_back) = other_b.split_at(mid);
            debug_assert_eq!(self_a_front.len(), other_a.len());
            debug_assert_eq!(self_a_mid.len(), other_b_mid.len());
            debug_assert_eq!(self_b.len(), other_b_back.len());
            self_a_front == other_a && self_a_mid == other_b_mid && self_b == other_b_back
        }
    }
}

impl<T, U, const N: usize> PartialEq<[U]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &[U]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let (self_a, self_b) = self.as_slices();
        let (other_a, other_b) = other.split_at(self_a.len());
        self_a == other_a && self_b == other_b
    }
}

impl<T, U, const N: usize> PartialEq<&[U]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U]) -> bool {
        PartialEq::eq(self, *other)
    }
}

impl<T, U, const N: usize> PartialEq<&mut [U]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&mut [U]) -> bool {
        PartialEq::eq(self, &**other)
    }
}

impl<T, U, const N: usize, const M: usize> PartialEq<[U; M]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U; M]) -> bool {
        PartialEq::eq(self, other.as_slice())
    }
}

impl<T, U, const N: usize, const M: usize> PartialEq<&[U; M]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U; M]) -> bool {
        PartialEq::eq(self, other.as_slice())
    }
}

impl<T, U, const N: usize, const M: usize> PartialEq<&mut [U; M]> for InplaceDeque<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&mut [U; M]) -> bool {
        PartialEq::eq(self, other.as_slice())
    }
}

impl<T: Eq, const N: usize> Eq for InplaceDeque<T, N> {}

impl<T: PartialOrd, const N: usize> PartialOrd for InplaceDeque<T, N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Ord, const N: usize> Ord for InplaceDeque<T, N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T: hash::Hash, const N: usize> hash::Hash for InplaceDeque<T, N> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len);
        self.iter().for_each(|x| x.hash(state));
    }
}

impl<T, const N: usize> ops::Index<usize> for InplaceDeque<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Out of bounds acccess")
    }
}

impl<T, const N: usize> ops::IndexMut<usize> for InplaceDeque<T, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Out of bounds acccess")
    }
}

impl<T, const N: usize> FromIterator<T> for InplaceDeque<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut q = Self::new();
        let len = q
            .buf
            .iter_mut()
            .zip(iter)
            .map(|(dst, src)| dst.write(src))
            .count();
        q.len = len;
        q
    }
}

impl<T, const N: usize> IntoIterator for InplaceDeque<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a InplaceDeque<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut InplaceDeque<T, N> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for InplaceDeque<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T, const N: usize> From<InplaceVec<T, N>> for InplaceDeque<T, N> {
    #[inline]
    fn from(other: InplaceVec<T, N>) -> Self {
        let (buf, len) = unsafe { other.into_raw() };
        Self { buf, head: 0, len }
    }
}

impl<T, const N: usize> From<InplaceDeque<T, N>> for InplaceVec<T, N> {
    fn from(mut other: InplaceDeque<T, N>) -> Self {
        other.make_contiguous();
        let ptr = (&raw mut other.buf) as *mut T;
        let len = other.len;
        if other.head != 0 {
            unsafe { ptr::copy(ptr.add(other.head), ptr, len) };
        }
        let buf = unsafe { ptr::read(&other.buf) };
        mem::forget(other);
        unsafe { Self::from_raw(buf, len) }
    }
}

impl<T, const N: usize> From<[T; N]> for InplaceDeque<T, N> {
    fn from(value: [T; N]) -> Self {
        Self {
            buf: value.map(MaybeUninit::new),
            head: 0,
            len: N,
        }
    }
}

#[inline]
fn wrap_index<const CAPACITY: usize>(logical_index: usize) -> usize {
    debug_assert!(
        (logical_index == 0 && CAPACITY == 0)
            || logical_index < CAPACITY
            || (logical_index - CAPACITY) < CAPACITY
    );
    if CAPACITY.is_power_of_two() {
        return logical_index & (CAPACITY - 1);
    }
    if logical_index >= CAPACITY {
        logical_index - CAPACITY
    } else {
        logical_index
    }
}

pub struct IsPow2Usize<const VALUE: usize>(());

macro_rules! is_pow2_impl {
    ($($n: literal),* $(,)?) => {
        $(
            impl $crate::assert::True for IsPow2Usize<{ 1 << $n }> {}
        )*
    };
}

is_pow2_impl!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_new() {
        let q = InplaceDeque::<i32, 128>::new();
        q.assert_pow2_capacity();
        let _q = InplaceDeque::<i32, 64>::with_pow2_capacity();
    }

    #[test]
    fn t_drain() {
        let mut q = InplaceDeque::from([1, 2, 3, 4, 5]);
        q.drain(1..3);
        assert_eq!(q, [1, 4, 5]);
    }
}
