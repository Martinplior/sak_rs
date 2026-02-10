pub mod complex;
pub mod matrix;
pub mod ops;
pub mod vector;

pub use complex::Complex2;
pub use matrix::{Matrix, mat};
pub use vector::{Vector, vec};

use std::time::{Duration, Instant};

pub fn fast_pow<T, MulFn, SqrFn>(
    mut base: T,
    exponent: core::num::NonZero<u64>,
    mut mul_fn: MulFn,
    mut sqr_fn: SqrFn,
) -> T
where
    T: Clone,
    MulFn: FnMut(&mut T, &T),
    SqrFn: FnMut(&mut T),
{
    let mut exponent = exponent.get();
    let mut result = loop {
        if exponent & 1 != 0 {
            break base.clone();
        }
        exponent >>= 1;
        sqr_fn(&mut base);
    };
    loop {
        if exponent == 1 {
            break;
        }
        exponent >>= 1;
        sqr_fn(&mut base);
        if exponent & 1 != 0 {
            mul_fn(&mut result, &base);
        }
    }
    result
}

/// Returns the next tick time based on the last tick time, current time, and interval.
/// ``` txt
/// |<---interval--->|                |                |                |
/// ^last_tick                  current_instant^       ^next_tick
/// ```
pub fn find_next_tick(
    last_tick: Instant,
    current_instant: Instant,
    interval: Duration,
) -> Option<Instant> {
    let passed_duration = current_instant.checked_duration_since(last_tick)?;
    if passed_duration < interval {
        return last_tick.checked_add(interval);
    }
    let next_duration = interval.mul_f64(passed_duration.div_duration_f64(interval).ceil());
    last_tick.checked_add(next_duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_pow() {
        for base in 1..=10_u32 {
            for exponent in 1..10 {
                let result = fast_pow(
                    base,
                    exponent.try_into().unwrap(),
                    |a, b| *a *= *b,
                    |x| *x *= *x,
                );
                assert_eq!(result, base.pow(exponent as u32));
                println!("{}**{} = {}", base, exponent, result)
            }
        }
    }
}
