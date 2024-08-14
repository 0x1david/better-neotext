use crate::buffer::TextBuffer;
use crossterm::{terminal, execute};

pub struct Editor<Buff: TextBuffer> {
    buffer: Buff
}

impl<Buff: TextBuffer> Drop for Editor<Buff> {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            stdout(),
            terminal::Clear(ClearType::All),
            LeaveAlternateScreen
        );
    }
}
impl<Buff: TextBuffer> Editor<Buff> {
    fn interpret_command(&self) {

    }
}
