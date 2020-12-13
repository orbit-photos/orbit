// #[derive(Debug, Clone, Copy)]
// pub struct Camera {
//     focal_length_millis: f32,
//     sensor_width_millis: f32,
//     image_width_pixels: f32,
//     image_height_pixels: f32,
// }
//
// impl Camera {
//     pub const IPHONE_6_LANDSCAPE_VIDEO: Camera = Camera {
//         focal_length_millis: 4.2,
//         sensor_width_millis: 4.8,
//         image_width_pixels: 1920.0,
//         image_height_pixels: 1080.0,
//     };
//     pub const IPHONE_6_PORTRAIT_VIDEO: Camera = Camera {
//         focal_length_millis: 4.2,
//         sensor_width_millis: 4.8,
//         image_width_pixels: 1080.0,
//         image_height_pixels: 1920.0,
//     };
//     pub const IPHONE_6_PORTRAIT_STILL: Camera = Camera {
//         focal_length_millis: 4.2,
//         sensor_width_millis: 4.8,
//         image_width_pixels: 2448.0,
//         image_height_pixels: 3264.0,
//     };
//     pub const INTEGRATED_WEBCAM: Camera = Camera {
//         focal_length_millis: 4.2,
//         sensor_width_millis: 4.8,
//         image_width_pixels: 640.0,
//         image_height_pixels: 480.0,
//     };
//     pub const SQ11: Camera = Camera {
//         focal_length_millis: 3.6,
//         sensor_width_millis: 7.056,
//         image_width_pixels: 1280.0,
//         image_height_pixels: 720.0,
//     };
//
//     pub fn focal_length_pixels(&self) -> f32 {
//         (self.focal_length_millis / self.sensor_width_millis) * self.image_width_pixels
//     }
//
//     pub fn image_width_pixels(&self) -> f32 {
//         self.image_width_pixels
//     }
//
//     pub fn image_height_pixels(&self) -> f32 {
//         self.image_height_pixels
//     }
//
//     /// Gets the size (in pixels) of an object with a certain angular size
//     pub fn vertical_size_pixels(&self, object_angular_height: f32) -> f32 {
//         self.image_height_pixels * object_angular_height / self.vertical_field_of_view()
//     }
//
//     fn horizontal_field_of_view(&self) -> f32 {
//         2.0 * (self.sensor_width_millis/self.focal_length_millis/2.0).atan()
//     }
//
//     fn vertical_field_of_view(&self) -> f32 {
//         self.horizontal_field_of_view() * self.image_height_pixels / self.image_width_pixels
//     }
// }

#[derive(Copy, Clone, Debug)]
pub struct CameraParameters {
    horizontal_field_of_view: f32,
    image_width_pixels: f32,
    image_height_pixels: f32,
}

impl CameraParameters {
    pub const SQ11: CameraParameters = CameraParameters {
        horizontal_field_of_view: 0.81425,
        image_width_pixels: 1280.0,
        image_height_pixels: 720.0,
    };

    pub fn focal_length_pixels(&self) -> f32 {
        0.5 * self.image_width_pixels / (self.horizontal_field_of_view * 0.5).tan()
    }

    /// Gets the size (in pixels) of an object with a certain angular size
    pub fn vertical_size_pixels(&self, object_angular_height: f32) -> f32 {
        self.image_height_pixels * object_angular_height / self.vertical_field_of_view()
    }
    pub fn horizontal_size_pixels(&self, object_angular_width: f32) -> f32 {
        self.image_width_pixels * object_angular_width / self.horizontal_field_of_view
    }

    pub fn image_width_pixels(&self) -> f32 {
        self.image_width_pixels
    }

    pub fn image_height_pixels(&self) -> f32 {
        self.image_height_pixels
    }

    fn horizontal_field_of_view(&self) -> f32 {
        self.horizontal_field_of_view
    }

    fn vertical_field_of_view(&self) -> f32 {
        self.horizontal_field_of_view * self.image_height_pixels / self.image_width_pixels
    }

}