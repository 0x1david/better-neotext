use std::io::stdout;

use crate::{buffer::TextBuffer, viewport::ViewPort, Action, Modal, Result};
use crossterm::{event::{self, Event, KeyEvent}, execute, terminal};

pub struct Editor<Buff: TextBuffer> {
    buffer: Buff,
    viewport: ViewPort,
    modal: Modal,
    action_history: Vec<Action>,
}

impl<Buff: TextBuffer> Editor<Buff> {
    fn run_event_loop(&self) -> Result<()>{
        loop {
            if let Event::Key(key_event) = event::read()? {
                let action = match self.modal {
                    Modal::Normal => self.interpret_normal_event(key_event),
                    Modal::Command => self.interpret_command_event(key_event),
                    _ => continue
                }?;
                self.perform_action(action)?
            }
        }
    }
    fn interpret_normal_event(&self, key_event: KeyEvent) -> Result<Action> {
        todo!()
    }
    fn interpret_command_event(&self, key_event: KeyEvent) -> Result<Action> {
        todo!()
    }
    fn perform_action(&self, action: Action) -> Result<()> {
        match action {
            Action::Quit => exit(1),
            Action::BumpUp => todo!(),
            Action::BumpDown => todo!(),
            Action::BumpLeft => todo!(),
            Action::BumpRight => todo!(),
        }

    }
}
