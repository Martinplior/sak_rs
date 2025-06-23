use std::ops;

pub fn range<R>(range: R, bounds: ops::RangeTo<usize>) -> Result<ops::Range<usize>, Box<str>>
where
    R: ops::RangeBounds<usize>,
{
    let len = bounds.end;

    let start = match range.start_bound() {
        ops::Bound::Included(&start) => start,
        ops::Bound::Excluded(start) => start.checked_add(1).ok_or_else(|| {
            let err = "attempted to index slice from after maximum usize".to_string();
            err.into_boxed_str()
        })?,
        ops::Bound::Unbounded => 0,
    };

    let end = match range.end_bound() {
        ops::Bound::Included(end) => end.checked_add(1).ok_or_else(|| {
            let err = "attempted to index slice up to maximum usize".to_string();
            err.into_boxed_str()
        })?,
        ops::Bound::Excluded(&end) => end,
        ops::Bound::Unbounded => len,
    };

    if start > end {
        let err = format!("slice index starts at {start} but ends at {end}",);
        return Err(err.into_boxed_str());
    }
    if end > len {
        let err = format!("range end index {end} out of range for slice of length {len}");
        return Err(err.into_boxed_str());
    }

    Ok(ops::Range { start, end })
}
