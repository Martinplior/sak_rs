#[cfg(feature = "os")]
pub mod os;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn t1() {}
}
