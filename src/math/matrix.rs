#[inline(always)]
pub fn mat<T, const ROWS: usize, const COLS: usize>(
    values: [[T; COLS]; ROWS],
) -> Matrix<T, ROWS, COLS> {
    Matrix::new(values)
}

macro_rules! impl_ops {
    ($op_name: ident, $op_fn_name: ident, $op: tt) => {
        impl<T, const ROWS: usize, const COLS: usize> core::ops::$op_name
            for &Matrix<T, ROWS, COLS>
        where
            for<'t> &'t T: core::ops::$op_name<Output = T>,
        {
            type Output = Matrix<T, ROWS, COLS>;

            #[inline]
            fn $op_fn_name(self, other: Self) -> Self::Output {
                Matrix::from_fn(|row, col| unsafe {
                    self.get_unchecked(row, col) $op other.get_unchecked(row, col)
                })
            }
        }

        impl<T, const ROWS: usize, const COLS: usize> core::ops::$op_name for Matrix<T, ROWS, COLS>
        where
            T: core::ops::$op_name<Output = T>,
        {
            type Output = Self;

            #[inline]
            fn $op_fn_name(self, other: Self) -> Self::Output {
                let a = core::mem::ManuallyDrop::new(self);
                let b = core::mem::ManuallyDrop::new(other);
                let a_ptr = a.0.as_ptr() as *const T;
                let b_ptr = b.0.as_ptr() as *const T;
                Matrix::from_fn(|row, col| unsafe {
                    let index = row * COLS + col;
                    a_ptr.add(index).read() $op b_ptr.add(index).read()
                })
            }
        }
    };
}

#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Matrix<T, const ROWS: usize, const COLS: usize>([[T; COLS]; ROWS]);

impl<T, const ROWS: usize, const COLS: usize> Matrix<T, ROWS, COLS> {
    #[inline(always)]
    pub fn new(values: [[T; COLS]; ROWS]) -> Self {
        assert!(ROWS != 0 && COLS != 0);
        Self(values)
    }

    /// `f(row, col)`
    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> T) -> Self {
        mat(core::array::from_fn(|row| {
            core::array::from_fn(|col| f(row, col))
        }))
    }

    #[inline(always)]
    pub fn get(&self, row: usize, col: usize) -> Option<&T> {
        self.0.get(row)?.get(col)
    }

    #[inline(always)]
    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut T> {
        self.0.get_mut(row)?.get_mut(col)
    }

    /// # Safety
    ///
    /// `row` and `col` must be valid indices in the matrix.
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, row: usize, col: usize) -> &T {
        unsafe { self.0.get_unchecked(row).get_unchecked(col) }
    }

    /// # Safety
    ///
    /// `row` and `col` must be valid indices in the matrix.
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, row: usize, col: usize) -> &mut T {
        unsafe { self.0.get_unchecked_mut(row).get_unchecked_mut(col) }
    }

    #[inline(always)]
    pub const fn rows(&self) -> usize {
        ROWS
    }

    #[inline(always)]
    pub const fn cols(&self) -> usize {
        COLS
    }

    #[inline(always)]
    pub const fn size(&self) -> usize {
        ROWS * COLS
    }

    #[inline(always)]
    pub fn into_array(self) -> [[T; COLS]; ROWS] {
        self.0
    }

    #[inline(always)]
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Matrix<U, ROWS, COLS> {
        mat(self.0.map(|row| row.map(&mut f)))
    }

    #[inline(always)]
    pub fn transpose(self) -> Matrix<T, COLS, ROWS> {
        let old = core::mem::ManuallyDrop::new(self);
        let ptr = old.as_ptr() as *const T;
        Matrix::from_fn(|row, col| unsafe { ptr.add(row + col * COLS).read() })
    }
}

macro_rules! impl_minor {
    ($main_edge: literal, $minor_edge: literal) => {
        impl<T> Matrix<T, $main_edge, $main_edge> {
            #[inline(always)]
            pub fn minor(&self, row: usize, col: usize) -> Matrix<T, $minor_edge, $minor_edge>
            where
                T: Clone,
            {
                Matrix::from_fn(|i, j| unsafe {
                    self.get_unchecked(
                        if i < row { i } else { i + 1 },
                        if j < col { j } else { j + 1 },
                    )
                    .clone()
                })
            }
        }
    };
}

impl_minor!(2, 1);
impl_minor!(3, 2);
impl_minor!(4, 3);

impl<T> Matrix<T, 1, 1> {
    #[inline(always)]
    pub fn determinant(&self) -> T
    where
        T: Clone,
    {
        unsafe { self.get_unchecked(0, 0).clone() }
    }
}

impl<T> Matrix<T, 2, 2> {
    #[inline(always)]
    pub fn determinant(&self) -> T
    where
        T: core::ops::Sub<Output = T> + Clone,
        for<'t> &'t T: core::ops::Mul<Output = T>,
    {
        unsafe {
            self.get_unchecked(0, 0) * self.get_unchecked(1, 1)
                - self.get_unchecked(0, 1) * self.get_unchecked(1, 0)
        }
    }
}

impl<T> Matrix<T, 3, 3> {
    pub fn determinant(&self) -> T
    where
        T: core::ops::Add<Output = T> + core::ops::Sub<Output = T> + core::ops::Mul<Output = T>,
        for<'t> &'t T: core::ops::Mul<Output = T>,
    {
        unsafe {
            self.get_unchecked(0, 0)
                * &(self.get_unchecked(1, 1) * self.get_unchecked(2, 2)
                    - self.get_unchecked(1, 2) * self.get_unchecked(2, 1))
                - self.get_unchecked(0, 1)
                    * &(self.get_unchecked(1, 0) * self.get_unchecked(2, 2)
                        - self.get_unchecked(1, 2) * self.get_unchecked(2, 0))
                + self.get_unchecked(0, 2)
                    * &(self.get_unchecked(1, 0) * self.get_unchecked(2, 1)
                        - self.get_unchecked(1, 1) * self.get_unchecked(2, 0))
        }
    }
}

impl<T> Matrix<T, 4, 4> {
    pub fn determinant(&self) -> T
    where
        T: core::ops::Add<Output = T> + core::ops::Sub<Output = T> + core::ops::Mul<Output = T>,
        for<'t> &'t T: core::ops::Mul<Output = T> + core::ops::Sub<Output = T>,
    {
        unsafe {
            let p_22_33 = self.get_unchecked(2, 2) * self.get_unchecked(3, 3);
            let p_23_32 = self.get_unchecked(2, 3) * self.get_unchecked(3, 2);
            let p_21_33 = self.get_unchecked(2, 1) * self.get_unchecked(3, 3);
            let p_23_31 = self.get_unchecked(2, 3) * self.get_unchecked(3, 1);
            let p_21_32 = self.get_unchecked(2, 1) * self.get_unchecked(3, 2);
            let p_22_31 = self.get_unchecked(2, 2) * self.get_unchecked(3, 1);
            let p_20_33 = self.get_unchecked(2, 0) * self.get_unchecked(3, 3);
            let p_23_30 = self.get_unchecked(2, 3) * self.get_unchecked(3, 0);
            let p_20_32 = self.get_unchecked(2, 0) * self.get_unchecked(3, 2);
            let p_22_30 = self.get_unchecked(2, 2) * self.get_unchecked(3, 0);
            let p_20_31 = self.get_unchecked(2, 0) * self.get_unchecked(3, 1);
            let p_21_30 = self.get_unchecked(2, 1) * self.get_unchecked(3, 0);

            let det_00 = self.get_unchecked(1, 1) * &(&p_22_33 - &p_23_32)
                - self.get_unchecked(1, 2) * &(&p_21_33 - &p_23_31)
                + self.get_unchecked(1, 3) * &(&p_21_32 - &p_22_31);

            let det_01 = self.get_unchecked(1, 0) * &(&p_22_33 - &p_23_32)
                - self.get_unchecked(1, 2) * &(&p_20_33 - &p_23_30)
                + self.get_unchecked(1, 3) * &(&p_20_32 - &p_22_30);

            let det_02 = self.get_unchecked(1, 0) * &(&p_21_33 - &p_23_31)
                - self.get_unchecked(1, 1) * &(&p_20_33 - &p_23_30)
                + self.get_unchecked(1, 3) * &(&p_20_31 - &p_21_30);

            let det_03 = self.get_unchecked(1, 0) * &(&p_21_32 - &p_22_31)
                - self.get_unchecked(1, 1) * &(&p_20_32 - &p_22_30)
                + self.get_unchecked(1, 2) * &(&p_20_31 - &p_21_30);

            self.get_unchecked(0, 0) * &det_00 - self.get_unchecked(0, 1) * &det_01
                + self.get_unchecked(0, 2) * &det_02
                - self.get_unchecked(0, 3) * &det_03
        }
    }
}

impl<T, const ROWS: usize, const COLS: usize> Default for Matrix<T, ROWS, COLS>
where
    T: Default,
{
    fn default() -> Self {
        mat(core::array::from_fn(|_| {
            core::array::from_fn(|_| T::default())
        }))
    }
}

impl<T, const ROWS: usize, const COLS: usize> core::ops::Deref for Matrix<T, ROWS, COLS> {
    type Target = [[T; COLS]; ROWS];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const ROWS: usize, const COLS: usize> core::ops::DerefMut for Matrix<T, ROWS, COLS> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, const ROWS: usize, const COLS: usize> core::fmt::Display for Matrix<T, ROWS, COLS>
where
    T: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "[")?;
        for (i, row) in self.0.iter().enumerate() {
            if i != 0 {
                write!(f, ",\n ")?;
            }
            write!(f, "[")?;
            for (j, val) in row.iter().enumerate() {
                if j != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", val)?;
            }
            write!(f, "]")?;
        }
        write!(f, "]")?;
        Ok(())
    }
}

impl<T, const M: usize, const N: usize, const P: usize> core::ops::Mul<&Matrix<T, N, P>>
    for &Matrix<T, M, N>
where
    T: core::ops::Add<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = Matrix<T, M, P>;

    fn mul(self, rhs: &Matrix<T, N, P>) -> Self::Output {
        Matrix::from_fn(|row, col| unsafe {
            (0..N)
                .map(|i| {
                    self.0.get_unchecked(row).get_unchecked(i)
                        * rhs.0.get_unchecked(i).get_unchecked(col)
                })
                .reduce(|acc, x| acc + x)
                .unwrap_unchecked()
        })
    }
}

impl<T, const M: usize, const N: usize, const P: usize> core::ops::Mul<Matrix<T, N, P>>
    for Matrix<T, M, N>
where
    T: core::ops::Add<Output = T>,
    for<'t> &'t T: core::ops::Mul<Output = T>,
{
    type Output = Matrix<T, M, P>;

    fn mul(self, rhs: Matrix<T, N, P>) -> Self::Output {
        (&self) * &rhs
    }
}

impl_ops!(Add, add, +);
impl_ops!(Sub, sub, -);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mat_display() {
        let m = mat([[1, 2, 3], [4, 5, 6]]);
        println!("{}", m);
        let m_transposed = m.transpose();
        println!("{}", m_transposed);
    }

    #[test]
    fn mat_mul() {
        let m1 = mat([[1, 2, 3, 4]]);
        let m2 = mat([[5], [6], [7]]);
        let m3 = m2 * m1;
        println!("{}", m3);
    }

    #[test]
    fn square_mat() {
        let m = mat([
            [1, 20, 3, 4],
            [50, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ]);
        println!("{}", m);
        let det = m.determinant();
        println!("det = {}", det);
        let m1 = m.minor(0, 1);
        println!("{}", m1);
    }
}
