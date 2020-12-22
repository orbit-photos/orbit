use image::{ImageBuffer, DynamicImage, Pixel};
use std::ops::{Deref, DerefMut};

pub trait ImageTransformExt {
    fn crop_rotate(&self, radians: f32, dst_width: f32, dst_height: f32) -> Self;
    // fn rotate(&self, radians: f32) -> Self;
}

impl ImageTransformExt for DynamicImage {
    fn crop_rotate(&self, radians: f32, dst_width: f32, dst_height: f32) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(ref image) => DynamicImage::ImageLuma8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageLumaA8(ref image) => DynamicImage::ImageLumaA8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageRgb8(ref image) => DynamicImage::ImageRgb8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageRgba8(ref image) => DynamicImage::ImageRgba8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageBgr8(ref image) => DynamicImage::ImageBgr8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageBgra8(ref image) => DynamicImage::ImageBgra8(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageLuma16(ref image) => DynamicImage::ImageLuma16(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageLumaA16(ref image) => DynamicImage::ImageLumaA16(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageRgb16(ref image) => DynamicImage::ImageRgb16(crop_rotate(image, radians, dst_width, dst_height)),
            DynamicImage::ImageRgba16(ref image) => DynamicImage::ImageRgba16(crop_rotate(image, radians, dst_width, dst_height)),
        }
    }
    
    // fn rotate(&self, radians: f32) -> DynamicImage {
    //     match self {
    //         DynamicImage::ImageLuma8(ref image) => DynamicImage::ImageLuma8(rotate(image, radians)),
    //         DynamicImage::ImageLumaA8(ref image) => DynamicImage::ImageLumaA8(rotate(image, radians)),
    //         DynamicImage::ImageRgb8(ref image) => DynamicImage::ImageRgb8(rotate(image, radians)),
    //         DynamicImage::ImageRgba8(ref image) => DynamicImage::ImageRgba8(rotate(image, radians)),
    //         DynamicImage::ImageBgr8(ref image) => DynamicImage::ImageBgr8(rotate(image, radians)),
    //         DynamicImage::ImageBgra8(ref image) => DynamicImage::ImageBgra8(rotate(image, radians)),
    //         DynamicImage::ImageLuma16(ref image) => DynamicImage::ImageLuma16(rotate(image, radians)),
    //         DynamicImage::ImageLumaA16(ref image) => DynamicImage::ImageLumaA16(rotate(image, radians)),
    //         DynamicImage::ImageRgb16(ref image) => DynamicImage::ImageRgb16(rotate(image, radians)),
    //         DynamicImage::ImageRgba16(ref image) => DynamicImage::ImageRgba16(rotate(image, radians)),
    //     }
    // }
}

/// # Preconditions
/// (dst_width, dst_height) must be <= crop_rotate_dimensions(image.width(), image.height(), radians)
fn crop_rotate<P: Pixel + 'static>(
    src: &ImageBuffer<P, Vec<P::Subpixel>>, 
    radians: f32,
    dst_width: f32,
    dst_height: f32,
) -> ImageBuffer<P, Vec<P::Subpixel>> {

    let src_width = src.width() as f32;
    let src_height = src.height() as f32;

    let src_center_x = src_width / 2.0 - 0.5;
    let src_center_y = src_height / 2.0 - 0.5;

    let (max_dst_width, max_dst_height) = crop_rotate_dimensions(src_width as f64, src_height as f64, radians as f64);
    let (max_dst_width, max_dst_height) = (max_dst_width as f32, max_dst_height as f32);
    assert!(dst_width <= max_dst_width);
    assert!(dst_height <= max_dst_height);
    assert_eq!(dst_width/dst_height, src_width/src_height, "aspect ratio doesn't match");

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

fn rotate<P: image::Pixel + 'static>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    radians: f32,
) -> ImageBuffer<P, Vec<P::Subpixel>> {

    let radians = std::f32::consts::PI+radians;
    let [m0, m1, m2, m3] = rotation_matrix(-radians);

    let channel_count: usize = <P as image::Pixel>::CHANNEL_COUNT as usize;

    let width_usize = image.width() as usize;

    let width = image.width() as f32;
    let height = image.height() as f32;

    let new_width = (m0*width).abs() + (m1*height).abs();
    let new_height = (m2*width).abs() + (m3*height).abs();

    let mut ret = ImageBuffer::new(new_width as u32, new_height as u32);

    let mut iy = 0;
    let mut y = 0.0;
    while iy < new_height as usize {
        let w0 = width/2.0 - m0*new_width/2.0 - m1*(y-new_height/2.0);
        let w1 = height/2.0 - m2*new_width/2.0 - m3*(y-new_height/2.0);

        let (x_min1, x_max1) =
            if m0 > 0.0 { (-w0/m0, (width-w0-1.0)/m0) }
            else { ((width-w0-1.0)/m0, -w0/m0) };

        let (x_min2, x_max2) =
            if m2 > 0.0 { (-w1/m2, (height-w1-1.0)/m2) }
            else { ((height-w1-1.0)/m2, -w1/m2) };

        let x_min = x_min1.max(x_min2).max(0.0);
        let x_max = x_max1.min(x_max2).min(new_width-1.0);

        let mut ix = x_min as usize;

        let mut source_x = m0*x_min + w0;
        let mut source_y = m2*x_min + w1;

        while ix < x_max as usize {
            let source_index = channel_count*(source_x as usize + width_usize*source_y as usize);
            let destination_index = channel_count*(ix + iy*new_width as usize);

            ret.deref_mut()[destination_index..destination_index+channel_count]
                .copy_from_slice(&image.deref()[source_index..source_index+channel_count]);

            source_x += m0;
            source_y += m2;

            ix += 1;
        }

        y += 1.0;
        iy += 1;
    }

    ret
}


pub fn crop_rotate_scale(frac_height_width: f64, radians: f64) -> f64 {
    if frac_height_width < 1.0 { // means width > height
        frac_height_width / (frac_height_width*radians.cos().abs() + radians.sin().abs())
    } else {
        1.0 / (frac_height_width*radians.sin().abs() + radians.cos().abs())
    }
}

pub fn crop_rotate_dimensions(src_width: f64, src_height: f64, radians: f64) -> (f64, f64) {
    let scale = crop_rotate_scale(src_height/src_width, radians);
    (scale*src_width, scale*src_height)
}

// pub fn crop_rotate_dimensions(src_width: f32, src_height: f32, radians: f32) -> (f32, f32) {
//     // TODO: fix because the two expressions in the if statements are identical
//
//     if src_width > src_height {
//         let dst_width = src_width * src_height / (src_height*radians.cos().abs() + src_width*radians.sin().abs());
//         let dst_height = dst_width * src_height / src_width;
//         // let dst_height = src_height * src_height / (src_height*radians.cos().abs() + src_width*radians.sin().abs());
//         (dst_width, dst_height)
//     } else {
//         let dst_width = src_width * src_width / (src_height*radians.sin().abs() + src_width*radians.cos().abs());
//         let dst_height = dst_width * src_height / src_width;
//         // let dst_height = src_width * src_height / (src_height*radians.sin().abs() + src_width*radians.cos().abs());
//         (dst_width, dst_height)
//     }
// }

pub fn rotation_matrix(t: f32) -> [f32; 4] {
    [
        t.cos(), -t.sin(),
        t.sin(), t.cos(),
    ]
}
