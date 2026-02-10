pub trait InnerProduct {
    type Output;

    fn inner_product(self, other: Self) -> Self::Output;
}

pub trait CrossProduct {
    type Output;

    fn cross_product(self, other: Self) -> Self::Output;
}
