use crate::{STREAM_ASPECT_HEIGHT, STREAM_ASPECT_WIDTH};
use crate::streams::{Streams, StreamOrdinal};
use glium::Rect;

pub struct LayoutEngine {
    window_width: u32,
    window_height: u32,
    horizontal_tile_count: u32,
    tile_width: u32,
    tile_height: u32,
    tile_count: u32,
}

impl LayoutEngine {
    pub fn new(window_width: u32, window_height: u32, tile_count: u32) -> LayoutEngine {
        /// Returns the smallest value of horizontal tile count
        fn horizontal_tile_count(tile_count: u32, window_width: u32, window_height: u32) -> u32 {
            for horizontal_tile_count in 1.. {
                let required_rows = ceiling_div(tile_count, horizontal_tile_count);
                let available_rows = horizontal_tile_count*window_height*STREAM_ASPECT_WIDTH/(window_width*STREAM_ASPECT_HEIGHT);
                if required_rows <= available_rows {
                    return horizontal_tile_count;
                }
            }

            unreachable!()
        }

        let (w, horizontal_tile_count) = (1..=window_width)
            .map(|window_width| (
                window_width,
                horizontal_tile_count(tile_count, window_width, window_height),
            ))
            .max_by_key(|&(window_width, horizontal_tile_count)| window_width/horizontal_tile_count)
            .unwrap();

        let tile_width = w/horizontal_tile_count;
        let tile_height = tile_width*STREAM_ASPECT_HEIGHT/STREAM_ASPECT_WIDTH;

        LayoutEngine {
            window_width,
            window_height,
            horizontal_tile_count,
            tile_width,
            tile_height,
            tile_count,
        }
    }

    pub fn update_stream_count(&mut self, new_tile_count: u32) {
        *self = LayoutEngine::new(self.window_width, self.window_height, new_tile_count);
    }

    pub fn update_screen_size(&mut self, new_window_width: u32, new_window_height: u32) {
        *self = LayoutEngine::new(new_window_width, new_window_height, self.tile_count);
    }

    pub fn cursor_is_over(&self, cursor_x: u32, cursor_y: u32, streams: &Streams) -> Option<StreamOrdinal> {
        let tile_x = cursor_x / self.tile_width;
        let tile_y = cursor_y / self.tile_height;
        let i = tile_y*self.horizontal_tile_count + tile_x;
        streams.get_ordinal(i as usize)
    }

    pub fn viewport_rect(&self, i: StreamOrdinal) -> Rect {
        let i = i.index() as u32;
        let tile_x = self.tile_width * (i % self.horizontal_tile_count);
        let tile_y = self.tile_height * (i / self.horizontal_tile_count);
        let tile_y = self.window_height - self.tile_height - tile_y;

        Rect {
            left: tile_x,
            bottom: tile_y,
            width: self.tile_width,
            height: self.tile_height,
        }
    }
}

fn integer_square_root(n: u32) -> u32 {
    (n as f64).sqrt() as u32
}

fn ceiling_div(a: u32, b: u32) -> u32 {
    a/b + if a%b == 0 { 0 } else { 1 }
}
