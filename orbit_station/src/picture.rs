use image::{ImageBuffer, DynamicImage, Pixel};

pub trait ImageTransformExt {
    fn crop_rotate(&self, radians: f32, crop_factor: f32) -> Self;
}

impl ImageTransformExt for DynamicImage {
    fn crop_rotate(&self, radians: f32, crop_factor: f32) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(ref image) => DynamicImage::ImageLuma8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageLumaA8(ref image) => DynamicImage::ImageLumaA8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageRgb8(ref image) => DynamicImage::ImageRgb8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageRgba8(ref image) => DynamicImage::ImageRgba8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageBgr8(ref image) => DynamicImage::ImageBgr8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageBgra8(ref image) => DynamicImage::ImageBgra8(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageLuma16(ref image) => DynamicImage::ImageLuma16(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageLumaA16(ref image) => DynamicImage::ImageLumaA16(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageRgb16(ref image) => DynamicImage::ImageRgb16(crop_rotate(image, radians, crop_factor)),
            DynamicImage::ImageRgba16(ref image) => DynamicImage::ImageRgba16(crop_rotate(image, radians, crop_factor)),
        }
    }
}

/// # Preconditions
/// (crop_factor) must be <= crop_rotate_dimensions(image.width(), image.height(), radians)
/// The aspect ratio of the source and destination images must match
fn crop_rotate<P: Pixel + 'static>(
    src: &ImageBuffer<P, Vec<P::Subpixel>>, 
    radians: f32,
    crop_factor: f32,
) -> ImageBuffer<P, Vec<P::Subpixel>> {

    let src_width = src.width() as f32;
    let src_height = src.height() as f32;

    let src_center_x = src_width / 2.0 - 0.5;
    let src_center_y = src_height / 2.0 - 0.5;
    
    assert!(crop_factor <= crop_rotate_scale(src.height() as f64/src.width() as f64, radians as f64) as f32);
    
    let dst_width = src_width * crop_factor;
    let dst_height = src_height * crop_factor;
    
    let dst_center_x = dst_width / 2.0;
    let dst_center_y = dst_height / 2.0;

    let [m0, m1, m2, m3] = rotation_matrix(radians);

    let x_offset = src_center_x - (m0*dst_center_x + m1*dst_center_y);
    let y_offset = src_center_y - (m2*dst_center_x + m3*dst_center_y);

    ImageBuffer::from_fn(dst_width as u32, dst_height as u32, |dst_x, dst_y| {
        let (dst_x, dst_y) = (dst_x as f32, dst_y as f32);

        let src_x = x_offset + m0*dst_x + m1*dst_y;
        let src_y = y_offset + m2*dst_x + m3*dst_y;

        *src.get_pixel(src_x as u32, src_y as u32)
    })
}

pub fn crop_rotate_scale(frac_height_width: f64, radians: f64) -> f64 {
    if frac_height_width < 1.0 { // means width > height
        frac_height_width / (frac_height_width*radians.cos().abs() + radians.sin().abs())
    } else {
        1.0 / (frac_height_width*radians.sin().abs() + radians.cos().abs())
    }
}

pub fn rotation_matrix(t: f32) -> [f32; 4] {
    [
        t.cos(), -t.sin(),
        t.sin(), t.cos(),
    ]
}
