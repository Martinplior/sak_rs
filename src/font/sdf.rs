//! Based on [tiny-sdf](https://github.com/mapbox/tiny-sdf)

#[derive(Debug, Clone)]
pub struct Sdf {
    pub bitmap: Box<[u8]>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct SdfGenerator {
    grid_outer: Box<[f32]>,
    grid_inner: Box<[f32]>,
    f: Box<[f32]>,
    v: Box<[u16]>,
    z: Box<[f32]>,
    grid_size: u32,
    edge_padding: u32,
    radius: f32,
    cutoff: f32,
}

impl SdfGenerator {
    pub fn new(edge_padding: u32, radius: f32, cutoff: f32) -> Self {
        Self {
            grid_outer: Box::new([]),
            grid_inner: Box::new([]),
            f: Box::new([]),
            v: Box::new([]),
            z: Box::new([]),
            grid_size: 0,
            edge_padding,
            radius,
            cutoff,
        }
    }

    #[inline]
    pub fn edge_padding(&self) -> u32 {
        self.edge_padding
    }

    #[inline]
    pub fn radius(&self) -> f32 {
        self.radius
    }

    #[inline]
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }

    pub fn generate(&mut self, bitmap: &[u8], width: u32) -> Sdf {
        if bitmap.is_empty() {
            return Sdf {
                bitmap: Box::new([]),
                width: 0,
                height: 0,
            };
        }

        debug_assert!((bitmap.len() as u32).is_multiple_of(width));

        let height = bitmap.len() as u32 / width;

        let padded_width = width + 2 * self.edge_padding;
        let padded_height = height + 2 * self.edge_padding;

        debug_assert!(padded_width <= u32::MAX / padded_height);

        let least_size = self.grid_size.max(padded_width.max(padded_height));

        if least_size > self.grid_size {
            self.grow(least_size);
        }

        self.grid_outer.fill(Self::INF);
        self.grid_inner.fill(0.0);

        for y in 0..height {
            for x in 0..width {
                let a = *unsafe { bitmap.get((y * width + x) as usize).unwrap_unchecked() };
                if a == 0 {
                    continue;
                }

                let j = ((y + self.edge_padding) * padded_width + x + self.edge_padding) as usize;

                let outer = unsafe { self.grid_outer.get_mut(j).unwrap_unchecked() };
                let inner = unsafe { self.grid_inner.get_mut(j).unwrap_unchecked() };
                if a == 255 {
                    *outer = 0.0;
                    *inner = Self::INF;
                } else {
                    let d = 0.5 - a as f32 / 255.0;
                    *outer = if d > 0.0 { d * d } else { 0.0 };
                    *inner = if d < 0.0 { d * d } else { 0.0 };
                }
            }
        }

        self.edt_2d::<false>(0, 0, padded_width, padded_height, padded_width);
        self.edt_2d::<true>(
            self.edge_padding,
            self.edge_padding,
            width,
            height,
            padded_width,
        );

        self.grid_outer
            .iter_mut()
            .filter(|v| **v == Self::INF)
            .for_each(|v| *v = self.radius * self.radius);

        let len = (padded_width * padded_height) as usize;
        let data = (0..len)
            .map(|i| {
                let outer = unsafe { self.grid_outer.get(i).unwrap_unchecked() };
                let inner = unsafe { self.grid_inner.get(i).unwrap_unchecked() };
                let d = outer.sqrt() - inner.sqrt();
                (255.0 - 255.0 * (d / self.radius + self.cutoff))
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect();

        Sdf {
            bitmap: data,
            width: padded_width,
            height: padded_height,
        }
    }
}

impl SdfGenerator {
    const INF: f32 = 1e30;

    fn grow(&mut self, new_size: u32) {
        self.grid_size = new_size;
        self.grid_outer = vec![0.0; (new_size * new_size) as usize].into_boxed_slice();
        self.grid_inner = vec![0.0; (new_size * new_size) as usize].into_boxed_slice();
        self.f = vec![0.0; new_size as usize].into_boxed_slice();
        self.v = vec![0; new_size as usize].into_boxed_slice();
        self.z = vec![0.0; new_size as usize + 1].into_boxed_slice();
    }

    fn edt_2d<const IS_INNER: bool>(
        &mut self,
        x0: u32,
        y0: u32,
        width: u32,
        height: u32,
        grid_size: u32,
    ) {
        for x in x0..(x0 + width) {
            self.edt_1d::<IS_INNER>(
                (y0 * grid_size + x) as usize,
                grid_size as usize,
                height as usize,
            );
        }
        for y in y0..(y0 + height) {
            self.edt_1d::<IS_INNER>((y * grid_size + x0) as usize, 1, width as usize);
        }
    }

    fn edt_1d<const IS_INNER: bool>(&mut self, offset: usize, stride: usize, length: usize) {
        let grid = if IS_INNER {
            &mut self.grid_inner
        } else {
            &mut self.grid_outer
        };

        let f = &mut self.f;
        let v = &mut self.v;
        let z = &mut self.z;

        *unsafe { v.get_mut(0).unwrap_unchecked() } = 0;
        *unsafe { z.get_mut(0).unwrap_unchecked() } = -Self::INF;
        *unsafe { z.get_mut(1).unwrap_unchecked() } = Self::INF;
        *unsafe { f.get_mut(0).unwrap_unchecked() } =
            *unsafe { grid.get(offset).unwrap_unchecked() };

        let mut k: isize = 0;
        let mut s: f32;
        for q in 1..length {
            *unsafe { f.get_mut(q).unwrap_unchecked() } =
                *unsafe { grid.get(offset + q * stride).unwrap_unchecked() };
            let q2 = (q * q) as f32;
            loop {
                let vk = *unsafe { v.get(k as usize).unwrap_unchecked() } as usize;
                let fq = *unsafe { f.get(q).unwrap_unchecked() };
                let fvk = *unsafe { f.get(vk).unwrap_unchecked() };
                s = (fq - fvk + q2 - (vk * vk) as f32) / (q - vk) as f32 * 0.5;
                let zk = *unsafe { z.get(k as usize).unwrap_unchecked() };
                if s <= zk {
                    k -= 1;
                    if k > -1 {
                        continue;
                    }
                }
                break;
            }

            k += 1;
            *unsafe { v.get_mut(k as usize).unwrap_unchecked() } = q as u16;
            *unsafe { z.get_mut(k as usize).unwrap_unchecked() } = s;
            *unsafe { z.get_mut(k as usize + 1).unwrap_unchecked() } = Self::INF;
        }

        let mut k = 0;
        for q in 0..length {
            while *unsafe { z.get(k + 1).unwrap_unchecked() } < q as f32 {
                k += 1;
            }

            let vk = *unsafe { v.get(k).unwrap_unchecked() };
            let qr = q as isize - vk as isize;
            let g = unsafe { grid.get_mut(offset + q * stride).unwrap_unchecked() };
            let fvk = *unsafe { f.get(vk as usize).unwrap_unchecked() };
            *g = (qr * qr) as f32 + fvk;
        }
    }
}
