use std::env;

mod load;
mod util;
pub use load::{load, Index};

fn main() {
    let args: Vec<String> = env::args().collect();
    let filename = args.get(1).expect("must provide one filename");

    let index = load(&filename).unwrap();
}
