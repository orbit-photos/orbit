use image::{DynamicImage, Rgba, GenericImageView};

pub struct Target {
    x: u32,
    y: u32,
}

pub fn from_matrix(target_candidates: &DynamicImage, dark_threshhold: u8) -> Vec<Target> {
    // lets iterate through all of the `dark` pixels

    let pixel_is_dark = |pixel: Rgba<u8>| {
        let rgb = pixel.0;
        rgb[0]/3 + rgb[1]/3 + rgb[2]/3 < dark_threshhold
    };

    let is_dark = |x: u32, y: u32| pixel_is_dark(target_candidates.get_pixel(x, y));

    let (width, height) = target_candidates.dimensions();

    let mut stack = Vec::new();

    let mut targets = Vec::new(); // we add coordinates of the targets here

    let mut has_seen= HasSeen::new(width as usize, height as usize);

    for y in 0..height {
        for x in 0..width {
            if !is_dark(x, y) || has_seen.get(x, y) { continue } // this pixel isn't a target, or we've already seen it

            if let Some(target) = flood_fill(x, y, width, height, &is_dark, &mut has_seen, &mut stack) {
                targets.push(target);
            }
        }
    }

    targets
}

fn flood_fill(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    is_dark: &impl Fn(u32, u32) -> bool,
    has_seen: &mut HasSeen,
    stack: &mut Vec<(u32, u32)>
) -> Option<Target> {
    // returns the topmost, rightmost, bottommost, leftmost point, and the total pixels filled

    assert!(!has_seen.get(x, y));
    assert!(is_dark(x, y));

    let mut top = y;
    let mut bottom = y;
    let mut left = x;
    let mut right = x;

    let mut x_sum = 0;
    let mut y_sum = 0;

    let mut pixels_filled = 0;

    stack.clear();
    stack.push((x, y));

    while let Some((x, y)) = stack.pop() {
        if has_seen.get(x, y) { continue } // after this was pushed on, this pixel was colored
        has_seen.mark_seen(x, y);

        pixels_filled += 1;

        x_sum += x;
        y_sum += y;

        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);


        for &(new_x, new_y) in &[(x.wrapping_sub(1), y), (x, y.wrapping_sub(1)), (x+1, y), (x, y+1)] {
            if new_x < width && new_y < height && is_dark(new_x, new_y) {
                stack.push((new_x, new_y));
            }
        }
    }

    let mean_x = x_sum / pixels_filled;
    let mean_y = y_sum / pixels_filled;

    Some(Target { x: mean_x, y: mean_y })
}

struct HasSeen {
    width: usize,
    height: usize,
    data: Vec<bool>,
}

impl HasSeen {
    fn new(width: usize, height: usize) -> HasSeen {
        HasSeen {
            width,
            height,
            data: vec![false; width*height],
        }
    }

    fn get(&self, x: u32, y: u32) -> bool {
        self.data[x as usize + y as usize*self.width]
    }

    fn mark_seen(&mut self, x: u32, y: u32) {
        self.data[x as usize + y as usize*self.width] = true;
    }
}