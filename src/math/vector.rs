#[inline(always)]
pub fn vec<T, const N: usize>(values: [T; N]) -> Vector<T, N> {
    Vector::new(values)
}

macro_rules! impl_ops {
    ($op_name: ident, $op_fn_name: ident, $op: tt) => {
        impl<T, const N: usize> core::ops::$op_name for &Vector<T, N>
        where
            for<'t> &'t T: core::ops::$op_name<Output = T>,
        {
            type Output = Vector<T, N>;

            #[inline]
            fn $op_fn_name(self, other: Self) -> Self::Output {
                Vector::from_fn(|i| unsafe {
                    self.0.get_unchecked(i) $op other.0.get_unchecked(i)
                })
            }
        }

        impl<T, const N: usize> core::ops::$op_name for Vector<T, N>
        where
            T: core::ops::$op_name<Output = T>,
        {
            type Output = Self;

            #[inline]
            fn $op_fn_name(self, other: Self) -> Self::Output {
                let a = core::mem::ManuallyDrop::new(self);
                let b = core::mem::ManuallyDrop::new(other);
                let a_ptr = a.0.as_ptr();
                let b_ptr = b.0.as_ptr();
                Vector::from_fn(|i| unsafe {
                    a_ptr.add(i).read() $op b_ptr.add(i).read()
                })
            }
        }
    };
}

macro_rules! impl_ops_single {
    ($op_name: ident, $op_fn_name: ident, $op: tt) => {
        impl<T, const N: usize> core::ops::$op_name for &Vector<T, N>
        where
            for<'t> &'t T: core::ops::$op_name<Output = T>,
        {
            type Output = Vector<T, N>;

            #[inline]
            fn $op_fn_name(self) -> Self::Output {
                Vector::from_fn(|i| unsafe {
                    $op self.0.get_unchecked(i)
                })
            }
        }

        impl<T, const N: usize> core::ops::$op_name for Vector<T, N>
        where
            T: core::ops::$op_name<Output = T>,
        {
            type Output = Self;

            #[inline]
            fn $op_fn_name(self) -> Self::Output {
                let a = core::mem::ManuallyDrop::new(self);
                let a_ptr = a.0.as_ptr();
                Vector::from_fn(|i| unsafe {
                    $op a_ptr.add(i).read()
                })
            }
        }
    };
}

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vector<T, const N: usize>([T; N]);

impl<T, const N: usize> Vector<T, N> {
    #[inline(always)]
    pub fn new(values: [T; N]) -> Self {
        assert!(N != 0);
        Self(values)
    }

    #[inline(always)]
    pub fn from_fn(f: impl FnMut(usize) -> T) -> Self {
        vec(core::array::from_fn(f))
    }

    #[inline(always)]
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Vector<U, N> {
        vec(self.0.map(f))
    }
}

impl<T, const N: usize> Default for Vector<T, N>
where
    T: Default,
{
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T, const N: usize> AsRef<Self> for Vector<T, N> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T, const N: usize> core::ops::Deref for Vector<T, N> {
    type Target = [T; N];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const N: usize> core::ops::DerefMut for Vector<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, const N: usize> From<[T; N]> for Vector<T, N> {
    #[inline(always)]
    fn from(values: [T; N]) -> Self {
        Self::new(values)
    }
}

impl<T, const N: usize> From<Vector<T, N>> for [T; N] {
    #[inline(always)]
    fn from(v: Vector<T, N>) -> Self {
        v.0
    }
}

impl_ops!(Add, add, +);
impl_ops!(Sub, sub, -);
impl_ops!(Mul, mul, *);
impl_ops!(Div, div, /);
impl_ops!(Rem, rem, %);
impl_ops!(BitAnd, bitand, &);
impl_ops!(BitOr, bitor, |);
impl_ops!(BitXor, bitxor, ^);
impl_ops!(Shl, shl, <<);
impl_ops!(Shr, shr, >>);
impl_ops_single!(Neg, neg, -);
impl_ops_single!(Not, not, !);

impl<T, const N: usize> super::ops::InnerProduct for &Vector<T, N>
where
    T: core::ops::Add<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = T;

    fn inner_product(self, other: Self) -> Self::Output {
        assert!(N != 0);
        let r = self
            .0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| a * b)
            .reduce(|acc, x| acc + x);
        unsafe { r.unwrap_unchecked() }
    }
}

impl<T, const N: usize> super::ops::InnerProduct for Vector<T, N>
where
    T: core::ops::Add<Output = T> + core::ops::Mul<Output = T>,
{
    type Output = T;

    fn inner_product(self, other: Self) -> Self::Output {
        assert!(N != 0);
        let r = self
            .0
            .into_iter()
            .zip(other.0)
            .map(|(a, b)| a * b)
            .reduce(|acc, x| acc + x);
        unsafe { r.unwrap_unchecked() }
    }
}

impl<T> super::ops::CrossProduct for &Vector<T, 2>
where
    T: core::ops::Sub<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = T;

    fn cross_product(self, other: Self) -> Self::Output {
        &self.0[0] * &other.0[1] - &self.0[1] * &other.0[0]
    }
}

impl<T> super::ops::CrossProduct for Vector<T, 2>
where
    T: core::ops::Sub<Output = T> + core::ops::Mul<Output = T>,
{
    type Output = T;

    fn cross_product(self, other: Self) -> Self::Output {
        let [x1, y1] = self.0;
        let [x2, y2] = other.0;
        x1 * y2 - y1 * x2
    }
}

#[cfg(test)]
mod tests {
    use crate::math::ops::InnerProduct;

    use super::*;

    #[test]
    fn test_1() {
        let a = vec([1, 6, 3, 8]);
        let b = vec([5, 2, 7, 4]);
        let a_add_b = &a + &b;
        let a_sub_b = &a - &b;
        let a_mul_b = &a * &b;
        let a_div_b = &a / &b;
        let a_rem_b = &a % &b;
        let a_bitand_b = &a & &b;
        let a_bitor_b = &a | &b;
        let a_bitxor_b = &a ^ &b;
        let a_shl_b = &a << &b;
        let a_shr_b = &a >> &b;
        let a_dot_b = a.as_ref().inner_product(&b);
        let a_neg = -&a;
        let a_not = !&a;
        println!("{:?} + {:?} = {:?}", a, b, a_add_b);
        println!("{:?} - {:?} = {:?}", a, b, a_sub_b);
        println!("{:?} * {:?} = {:?}", a, b, a_mul_b);
        println!("{:?} / {:?} = {:?}", a, b, a_div_b);
        println!("{:?} % {:?} = {:?}", a, b, a_rem_b);
        println!("{:?} & {:?} = {:?}", a, b, a_bitand_b);
        println!("{:?} | {:?} = {:?}", a, b, a_bitor_b);
        println!("{:?} ^ {:?} = {:?}", a, b, a_bitxor_b);
        println!("{:?} << {:?} = {:?}", a, b, a_shl_b);
        println!("{:?} >> {:?} = {:?}", a, b, a_shr_b);
        println!("{:?} dot {:?} = {:?}", a, b, a_dot_b);
        println!("-{:?} = {:?}", a, a_neg);
        println!("!{:?} = {:?}", a, a_not);
    }

    #[test]
    fn test_3() {
        let _ = vec([0; 0]);
    }
}
