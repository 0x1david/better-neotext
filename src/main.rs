mod common;
mod editor;
mod error;
mod buffer;
mod viewport;
mod cursor;
use buffer::VecBuffer;
pub use common::*;

fn main() {
    let mut instance = editor::Editor::new(VecBuffer::new(vec![" ".to_string()]), false);
    let _ = instance.run_event_loop();
}
