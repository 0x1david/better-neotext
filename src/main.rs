mod buffer;
mod common;
mod cursor;
mod editor;
mod error;
mod viewport;
use buffer::VecBuffer;
pub use common::*;

fn main() {
    let mut instance = editor::Editor::new(VecBuffer::new(vec![" ".to_string()]), false);
    instance.run_event_loop().unwrap();
}
