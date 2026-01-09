use std::{num::NonZero, sync::Arc};

use vulkano::{
    buffer::Subbuffer,
    command_buffer::{
        AutoCommandBufferBuilder, BlitImageInfo, BufferImageCopy, CopyBufferToImageInfo, ImageBlit,
    },
    image::{Image, ImageCreateInfo, ImageSubresourceLayers, ImageType, sampler::Filter},
    memory::allocator::{AllocationCreateInfo, MemoryAllocator},
};

/// `image_type`, `extent[2]`, `mip_levels` in `create_info` will be ignored.
///
/// use auto mip levels if `desire_mip_levels` is `None`.
pub fn create_image2d_with_mipmaps<L>(
    allocator: Arc<dyn MemoryAllocator>,
    create_info: ImageCreateInfo,
    allocation_info: AllocationCreateInfo,
    desire_mip_levels: Option<NonZero<u32>>,
    main_level_buffer: Subbuffer<impl ?Sized>,
    command_builder: &mut AutoCommandBufferBuilder<L>,
) -> Arc<Image> {
    let extent = [create_info.extent[0], create_info.extent[1], 1];
    let mip_levels = if let Some(mip_levels) = desire_mip_levels {
        mip_levels.get()
    } else {
        ((extent[0].max(extent[1]) as f32).log2().floor() + 1.0) as u32
    };
    let image = Image::new(
        allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            extent,
            mip_levels,
            ..create_info
        },
        allocation_info,
    )
    .expect("Failed to create image");

    let main_region = BufferImageCopy {
        image_extent: extent,
        image_subresource: ImageSubresourceLayers {
            mip_level: 0,
            ..image.subresource_layers()
        },
        ..Default::default()
    };
    command_builder
        .copy_buffer_to_image(CopyBufferToImageInfo {
            regions: [main_region.clone()].into_iter().collect(),
            ..CopyBufferToImageInfo::buffer_image(main_level_buffer, image.clone())
        })
        .expect("Failed to copy buffer to image");

    (1..mip_levels).for_each(|i| {
        let default_blit_info = BlitImageInfo::images(image.clone(), image.clone());
        let default_region = default_blit_info.regions.first().expect("unreachable");
        let region = ImageBlit {
            src_subresource: ImageSubresourceLayers {
                mip_level: i - 1,
                ..default_region.src_subresource.clone()
            },
            src_offsets: [[0; 3], [extent[0] >> (i - 1), extent[1] >> (i - 1), 1]],
            dst_subresource: ImageSubresourceLayers {
                mip_level: i,
                ..default_region.dst_subresource.clone()
            },
            dst_offsets: [[0; 3], [extent[0] >> i, extent[1] >> i, 1]],
            ..Default::default()
        };
        command_builder
            .blit_image(BlitImageInfo {
                regions: [region].into_iter().collect(),
                filter: Filter::Linear,
                ..default_blit_info
            })
            .expect("Failed to blit image");
    });

    image
}
