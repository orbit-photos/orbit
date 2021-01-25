
mod lines;

fn main() {
    let input = image::open("detect/resources/stripe5_small1.png").unwrap();
    let gray = input.to_luma8();

    lines::heap(&gray);
}
