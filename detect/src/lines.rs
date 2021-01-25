use image::{GrayImage, Rgb, RgbImage};
use std::cmp::{Ordering};
use std::collections::BinaryHeap;
use arrayvec::{ArrayVec};
use image::buffer::ConvertBuffer;
use std::convert::TryFrom;


const MASK: [f32; 5] = [1.0, 1.0, 0.0, -1.0, -1.0];
const HALF_MASK: u32 = MASK.len() as u32 / 2;

// const IDEAL_RATIOS: [f32; 5] = [2.0, 1.0, 1.0, 1.0, 4.0]; // top to bottom STRIPE4
// const IDEAL_RATIOS: [f32; 5] = [4.0, 1.0, 1.0, 1.0, 2.0]; // top to bottom STRIPE4
const IDEAL_RATIOS: [f32; 9] = [8.0, 1.0, 2.0, 1.0, 1.0, 1.0, 4.0, 1.0, 16.0]; // long thingy
const SEGMENT_COUNT: usize = IDEAL_RATIOS.len() + 1;

const START_TRANSITION: TransitionKind = TransitionKind::BlackToWhite; // CHANGE

const MIN_STARKNESS: f32 = 0.2;
const MAX_ENTRY_LOSS: f32 = 0.2;
const MAX_ENTRY_DIFF: usize = 50;

pub fn heap(image: &GrayImage) {
    let transitions = Transitions::from_gray_image(image);

    transitions.draw_on(image).save("eleven.png").unwrap();

    let mut all = Vec::new();

    for (x, col) in transitions.columns.iter().enumerate() {
        let mut heap = BinaryHeap::new();

        heap.extend(col.iter().enumerate()
            .filter(|(_, t)| t.kind() == START_TRANSITION)
            .map(|(y, &transition)| Entry::new(transition, x, y))
        );

        while let Some(entry) = heap.pop() {
            let mut skipped = 0.0;

            for (y, &next_transition) in col.iter().enumerate().skip(entry.last_index()+1) { // skip additional because its not going to be looking at
                match entry.clone().next(next_transition, y, skipped) {
                    Result::NotValid => {},
                    Result::Completed(found) => all.push(found),
                    Result::Push(next) => heap.push(next),
                }

                skipped += 1.0;
            }
        }
    }

    let target_candidates: Vec<_> = all.into_iter()
        .filter(|e| e.loss() < MAX_ENTRY_LOSS) // todo: better way to estimate cutoff
        .collect();


    display(image, &target_candidates).save("fourteen.png").unwrap();
    dbg!(two_targets(target_candidates));

}

#[derive(Debug)]
struct Target {
    mean_x: usize,
    mean_y: usize,
}

impl Target {
    fn from_entries(entries: &[Entry]) -> Target {
        let mut x_sum = 0;
        let mut y_sum = 0;
        for entry in entries {
            x_sum += entry.x;
            y_sum += entry.mean_y();
        }

        Target {
            mean_x: x_sum / entries.len(),
            mean_y: y_sum / entries.len(),
        }
    }
}

fn two_targets(mut target_candidates: Vec<Entry>) -> Option<(Target, Target)> {
    target_candidates.sort_by_key(|e| e.mean_y());

    // find the biggest gap

    let (cutoff, diff) = target_candidates.windows(2).zip(1..)
        .filter_map(|(i, array)|<&[Entry; 2]>::try_from(array).ok().map(|e| (i, e)))
        .map(|(i, [e1, e2])| {
            let to_maximize: usize = e2.mean_y() - e1.mean_y();
            (i, to_maximize)
        })
        .max_by_key(|a| a.1).unwrap();

    if diff < MAX_ENTRY_DIFF {
        println!("failed to find two targets, diff was {}", diff);
        dbg!(cutoff);
        return None;
    }

    let target1 = Target::from_entries(&target_candidates[..cutoff]);
    let target2 = Target::from_entries(&target_candidates[cutoff..]);

    Some((target1, target2))
}


fn display(image: &GrayImage, entries: &[Entry]) -> RgbImage {
    let mut image: RgbImage = image.convert();

    for entry in entries {
        let x = entry.x as u32;

        let mut black = true;
        let mut iter = entry.transitions.iter().map(|t| t.y);
        let mut last_y = iter.next().unwrap();
        for end_y in iter {
            for y in last_y..end_y {
                let color =
                    if black { Rgb([255, 0, 0]) }
                    else { Rgb([0, 0, 255]) };

                image.put_pixel(x, y as u32, color);
            }

            last_y = end_y;

            black = !black;
        }
    }

    image

}

enum Result {
    Completed(Entry),
    Push(Entry),
    NotValid,
}

#[derive(Clone)]
struct Entry {
    transitions: ArrayVec<[Transition; SEGMENT_COUNT]>,
    skipped_error: f32,
    x: usize,
    last_y: usize,
}

impl Entry {
    fn new(transition: Transition, x: usize, y: usize) -> Entry {
        let mut transitions = ArrayVec::new();
        transitions.push(transition);
        Entry { transitions, x, last_y: y, skipped_error: 0.0 }
    }

    fn next(mut self, found: Transition, y: usize, skipped_starkness_sum: f32) -> Result {
        if self.looking_for() != found.kind() { return Result::NotValid }

        self.transitions.push(found);

        self.skipped_error += skipped_starkness_sum;
        self.last_y = y;

        if self.transitions.is_full() {
            Result::Completed(self)
        } else {
            Result::Push(self)
        }
    }

    fn proportion_error(&self) -> f32 {
        let mut ys = self.transitions.iter().map(|t| t.y);

        let mut last_y = ys.next().unwrap();

        let fractions_of_ideal: ArrayVec<[f32; IDEAL_RATIOS.len()]> = ys.zip(IDEAL_RATIOS.iter())
            .map(|(y, &ideal)| {
                let section_width = y - last_y;
                last_y = y;
                section_width as f32 / ideal
            })
            .collect();

        let mean_pixels_per_section = fractions_of_ideal.iter().copied().sum::<f32>() / fractions_of_ideal.len() as f32;

        let square_error_sum: f32 = fractions_of_ideal.iter()
            .map(|fraction_of_ideal| {
                let error = fraction_of_ideal/mean_pixels_per_section - 1.0; // should be 0
                error*error
            })
            .sum();

        let mean_square_error = square_error_sum / fractions_of_ideal.len() as f32;

        mean_square_error
    }

    fn looking_for(&self) -> TransitionKind {
        // if self.transitions.len() % 2 == 0 { CHANGE
        if self.transitions.len() % 2 == 1 {
            TransitionKind::WhiteToBlack
        } else {
            TransitionKind::BlackToWhite
        }
    }

    fn last_index(&self) -> usize {
        self.last_y
    }

    fn loss(&self) -> f32 {
        let proportion_error = self.proportion_error();
        proportion_error + self.skipped_error
    }

    fn mean_y(&self) -> usize {
        let mut y_sum = 0;
        let mut y_count = 0;

        for array in self.transitions.windows(2) { // TODO: use array_windows
            if let [y_start, y_end] = array {
                for y in y_start.y..y_end.y { // TODO: use math instead of for loop
                    y_sum += y;
                    y_count += 1;
                }
            }
        }

        y_sum / y_count
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Entry) -> bool {
        self.loss() == other.loss()
    }
}

impl Eq for Entry { }

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Entry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn total_cmp(a: f32, b: f32) -> Ordering {
    let mut left = a.to_bits() as i32;
    let mut right = b.to_bits() as i32;

    left ^= (((left >> 31) as u32) >> 1) as i32;
    right ^= (((right >> 31) as u32) >> 1) as i32;

    left.cmp(&right)
}

impl Ord for Entry {
    fn cmp(&self, other: &Entry) -> Ordering {
        total_cmp(self.loss(), other.loss()).reverse()
    }
}



#[derive(Copy, Clone)]
pub struct Transition {
    above_minus_below: f32, // -1 to 1
    y: usize,
}

impl Transition {
    fn from_pixel_iter(pixels: impl Iterator<Item=u8>, y: usize) -> Transition {
        let above_minus_below = pixels.zip(MASK.iter())
            .map(|(pixel, &mask)| pixel as f32 / 255.0 * mask)
            .sum();

        Transition { above_minus_below, y }
    }

    fn kind(&self) -> TransitionKind {
        if self.above_minus_below > 0.0 {
            TransitionKind::WhiteToBlack
        } else {
            TransitionKind::BlackToWhite
        }
    }

    fn starkness(&self) -> f32 {
        self.above_minus_below.abs()
    }
}

pub struct DedupState {
    max_transition: Transition,
    last_y: usize,
}

impl DedupState {
    fn new(transition: Transition) -> DedupState {
        DedupState {
            max_transition: transition,
            last_y: transition.y,
        }
    }

    fn add(&mut self, transition: Transition) -> Option<Transition> {
        if transition.starkness() < MIN_STARKNESS { return None }

        let mut ret = None;

        if self.last_y+1 == transition.y && transition.kind() == self.max_transition.kind() {
            if transition.starkness() > self.max_transition.starkness() {
                self.max_transition = transition;
            }

        } else {
            ret = Some(self.max_transition);
            self.max_transition = transition;
            self.last_y = transition.y;
        }

        self.last_y = transition.y;

        ret
    }
}

pub struct Transitions {
    columns: Vec<Vec<Transition>>,
}

impl Transitions {
    pub fn from_gray_image(image: &GrayImage) -> Transitions {
        let mut columns = Vec::with_capacity(image.width() as usize);

        for x in 0..image.width() {
            let mut column = Vec::new();

            let mut dedup_state = DedupState::new(Transition { above_minus_below: 0.0, y: 0 });

            for y in HALF_MASK..image.height()-HALF_MASK {
                let transition = Transition::from_pixel_iter(
                    (y-HALF_MASK..=y+HALF_MASK)
                            .map(|y| image.get_pixel(x, y).0[0]),
                    y as usize,
                );

                if let Some(max_state) = dedup_state.add(transition) {
                    column.push(max_state);
                }
            }

            columns.push(column);
        }

        Transitions { columns }
    }

    pub fn draw_on(&self, gray: &GrayImage) -> RgbImage {
        let mut ret: RgbImage = gray.convert();

        for (x, col) in self.columns.iter().enumerate() {
            for transition in col {
                let value = (transition.starkness() * 255.0) as u8;
                let pixel = match transition.kind() {
                    TransitionKind::WhiteToBlack => Rgb([value, 0, 0]),
                    TransitionKind::BlackToWhite => Rgb([0, value, 0]),
                };

                ret.put_pixel(x as u32, transition.y as u32, pixel);
            }
        }

        ret
    }
}


#[derive(Eq, PartialEq)]
pub enum TransitionKind {
    BlackToWhite,
    WhiteToBlack,
}
