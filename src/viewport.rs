use std::io::{stdout, Stdout};

use crossterm::{execute, terminal::{self, ClearType, LeaveAlternateScreen}};


pub struct ViewPort {
    terminal: Stdout
}

impl Drop for ViewPort {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            self.terminal,
            terminal::Clear(ClearType::All),
            LeaveAlternateScreen
        );
    }
}

impl Default for ViewPort {
    fn default() -> Self {
        terminal::enable_raw_mode().expect("Couldn't start up terminal in raw mode.");
        Self {
            terminal: stdout()
        }
    }
}
