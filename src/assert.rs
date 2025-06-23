pub trait True {}

pub trait False {}

pub struct Assert<const VALUE: bool>(());

impl True for Assert<true> {}

impl False for Assert<false> {}
