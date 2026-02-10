use super::Vector;

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Complex2<T>([T; 2]);

impl<T> Complex2<T> {
    #[inline(always)]
    pub fn new(values: [T; 2]) -> Self {
        Self(values)
    }

    #[inline(always)]
    pub fn real(&self) -> &T {
        &self.0[0]
    }

    #[inline(always)]
    pub fn real_mut(&mut self) -> &mut T {
        &mut self.0[0]
    }

    #[inline(always)]
    pub fn imag(&self) -> &T {
        &self.0[1]
    }

    #[inline(always)]
    pub fn imag_mut(&mut self) -> &mut T {
        &mut self.0[1]
    }
}

impl<T> From<Vector<T, 2>> for Complex2<T> {
    #[inline(always)]
    fn from(v: Vector<T, 2>) -> Self {
        Self(v.into())
    }
}

impl<T> From<Complex2<T>> for Vector<T, 2> {
    #[inline(always)]
    fn from(c: Complex2<T>) -> Self {
        c.0.into()
    }
}

impl<T> From<[T; 2]> for Complex2<T> {
    #[inline(always)]
    fn from(v: [T; 2]) -> Self {
        Self(v.into())
    }
}

impl<T> From<Complex2<T>> for [T; 2] {
    #[inline(always)]
    fn from(c: Complex2<T>) -> Self {
        c.0.into()
    }
}

impl<T> core::ops::Index<usize> for Complex2<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T> core::ops::IndexMut<usize> for Complex2<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl<T> core::ops::Add for &Complex2<T>
where
    for<'t> &'t T: core::ops::Add<Output = T>,
{
    type Output = Complex2<T>;

    #[inline(always)]
    fn add(self, other: Self) -> Self::Output {
        Complex2::new([self.real() + other.real(), self.imag() + other.imag()])
    }
}

impl<T> core::ops::Add for Complex2<T>
where
    T: core::ops::Add<Output = T>,
{
    type Output = Self;

    #[inline(always)]
    fn add(self, other: Self) -> Self::Output {
        let [x1, y1] = self.0.into();
        let [x2, y2] = other.0.into();
        Self::new([x1 + x2, y1 + y2])
    }
}

impl<T> core::ops::Sub for &Complex2<T>
where
    for<'t> &'t T: core::ops::Sub<Output = T>,
{
    type Output = Complex2<T>;

    #[inline(always)]
    fn sub(self, other: Self) -> Self::Output {
        Complex2::new([self.real() - other.real(), self.imag() - other.imag()])
    }
}

impl<T> core::ops::Sub for Complex2<T>
where
    T: core::ops::Sub<Output = T>,
{
    type Output = Self;

    #[inline(always)]
    fn sub(self, other: Self) -> Self::Output {
        let [x1, y1] = self.0.into();
        let [x2, y2] = other.0.into();
        Self::new([x1 - x2, y1 - y2])
    }
}

impl<T> core::ops::Mul for &Complex2<T>
where
    T: core::ops::Add<Output = T> + core::ops::Sub<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = Complex2<T>;

    #[inline(always)]
    fn mul(self, other: Self) -> Self::Output {
        let real = self.real() * other.real() - self.imag() * other.imag();
        let imag = self.real() * other.imag() + self.imag() * other.real();
        Complex2::new([real, imag])
    }
}

impl<T> core::ops::Mul for Complex2<T>
where
    T: core::ops::Add<Output = T> + core::ops::Sub<Output = T> + core::ops::Mul<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = Self;

    #[inline(always)]
    fn mul(self, other: Self) -> Self::Output {
        let [x1, y1] = self.0.into();
        let [x2, y2] = other.0.into();
        let real = &x1 * &x2 - &y1 * &y2;
        let imag = x1 * y2 + y1 * x2;
        Self::new([real, imag])
    }
}

impl<T> core::ops::Div for &Complex2<T>
where
    T: core::ops::Add<Output = T> + core::ops::Sub<Output = T> + core::ops::Div<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T> + core::ops::Div<Output = T>,
{
    type Output = Complex2<T>;

    #[inline(always)]
    fn div(self, other: Self) -> Self::Output {
        let denominator = other.real() * other.real() + other.imag() * other.imag();
        let real_numerator = self.real() * other.real() + self.imag() * other.imag();
        let imag_numerator = self.imag() * other.real() - self.real() * other.imag();
        let real = &real_numerator / &denominator;
        let imag = imag_numerator / denominator;
        Complex2::new([real, imag])
    }
}

impl<T> core::ops::Div for Complex2<T>
where
    T: core::ops::Add<Output = T>
        + core::ops::Sub<Output = T>
        + core::ops::Mul<Output = T>
        + core::ops::Div<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T> + core::ops::Div<Output = T>,
{
    type Output = Self;

    #[inline(always)]
    fn div(self, other: Self) -> Self::Output {
        let [x1, y1] = self.0.into();
        let [x2, y2] = other.0.into();
        let denominator = &x2 * &x2 + &y2 * &y2;
        let real_numerator = &x1 * &x2 + &y1 * &y2;
        let imag_numerator = y1 * x2 - x1 * y2;
        let real = &real_numerator / &denominator;
        let imag = imag_numerator / denominator;
        Self::new([real, imag])
    }
}

impl<T> core::ops::Neg for &Complex2<T>
where
    for<'t> &'t T: core::ops::Neg<Output = T>,
{
    type Output = Complex2<T>;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        Complex2::new([-self.real(), -self.imag()])
    }
}

impl<T> core::ops::Neg for Complex2<T>
where
    T: core::ops::Neg<Output = T>,
{
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        let [x, y] = self.0.into();
        Self::new([-x, -y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex2() {
        let a = Complex2::new([1.0, 2.0]);
        let b = Complex2::new([3.0, 4.0]);
        let a_add_b = &a + &b;
        let a_sub_b = &a - &b;
        let a_mul_b = &a * &b;
        let a_div_b = &a / &b;
        let a_neg = -&a;
        println!("{:?} + {:?} = {:?}", a, b, a_add_b);
        println!("{:?} - {:?} = {:?}", a, b, a_sub_b);
        println!("{:?} * {:?} = {:?}", a, b, a_mul_b);
        println!("{:?} / {:?} = {:?}", a, b, a_div_b);
        println!("-{:?} = {:?}", a, a_neg);
    }
}
