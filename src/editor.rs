use crate::{buffer::TextBuffer, cursor::Cursor, viewport::ViewPort, Action, Command, Component, Error, FindMode, Modal, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

pub struct Editor<Buff: TextBuffer> {
    buffer: Buff,
    viewport: ViewPort,
    modal: Modal,
    action_history: Vec<Action>,
    repeat_action: usize,
    previous_key: Option<char>,
    cursor: Cursor,
    extensions: Vec<Box<dyn Component>>
}

impl<Buff: TextBuffer> Editor<Buff> {
    fn run_event_loop(&mut self) -> Result<()>{
        loop {
            self.viewport.update_viewport(self.buffer.get_entire_text(), self.cursor.line());
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
    fn interpret_normal_event(&mut self, key_event: KeyEvent) -> Result<Action> {

        let action = if let Some(prev) = self.previous_key.take() {
            match (prev, key_event.code) {
                ('t', KeyCode::Char(c)) => Action::JumpLetter(c),
                ('T', KeyCode::Char(c)) => Action::ReverseJumpLetter(c),
                ('f', KeyCode::Char(c)) => Action::FindChar(c),
                ('F', KeyCode::Char(c)) => Action::ReverseFindChar(c),
                ('r', KeyCode::Char(c)) => Action::Replace(c),
                ('p', KeyCode::Char(c)) => Action::Paste(c),
                ('P', KeyCode::Char(c)) => Action::PasteAbove(c),
                _ => Action::Nothing
            }
        } else {
            match (key_event.code, key_event.modifiers) {
            // Cursor Movement
            (KeyCode::Char('k'), KeyModifiers::NONE) => Action::BumpUp,
            (KeyCode::Char('j'), KeyModifiers::NONE) => Action::BumpDown,
            (KeyCode::Char('h'), KeyModifiers::NONE) => Action::BumpLeft,
            (KeyCode::Char('l'), KeyModifiers::NONE) => Action::BumpRight,
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => Action::JumpUp,
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => Action::JumpDown,

            (KeyCode::Char('W'), KeyModifiers::NONE) => Action::JumpToNextWord,
            (KeyCode::Char('w'), KeyModifiers::NONE) => Action::JumpToNextSymbol,
            (KeyCode::Char('B'), KeyModifiers::NONE) => Action::ReverseJumpToNextWord,
            (KeyCode::Char('b'), KeyModifiers::NONE) => Action::ReverseJumpToNextSymbol,
            (KeyCode::Char('_'), KeyModifiers::NONE) => Action::JumpSOL,
            (KeyCode::Home, KeyModifiers::NONE) => Action::JumpSOL,
            (KeyCode::Char('$'), KeyModifiers::NONE) => Action::JumpEOL,
            (KeyCode::End, KeyModifiers::NONE) => Action::JumpEOL,
            (KeyCode::Char('g'), KeyModifiers::NONE) => Action::JumpSOF,
            (KeyCode::Char('G'), KeyModifiers::NONE) => Action::JumpEOF,

            // Mode Changes
            (KeyCode::Char('v'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Visual),
            (KeyCode::Char('V'), KeyModifiers::NONE) => Action::ChangeMode(Modal::VisualLine),
            (KeyCode::Char(':'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Command),
            (KeyCode::Char('A'), KeyModifiers::NONE) => Action::InsertModeEOL,

            // Text Search
            (KeyCode::Char('/'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Find(FindMode::Forwards)),
            (KeyCode::Char('?'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Find(FindMode::Backwards)),

            // Text Manipulation
            (KeyCode::Char('o'), KeyModifiers::NONE) => Action::InsertNewline,
            (KeyCode::Char('O'), KeyModifiers::NONE) => Action::InsertBelow,
            (KeyCode::Char('X'), KeyModifiers::NONE) => Action::DeleteBefore,
            (KeyCode::Char('x'), KeyModifiers::NONE) => Action::DeleteUnder,

            // Undo/Redo
            (KeyCode::Char('u'), KeyModifiers::NONE) => Action::Undo(1),
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => Action::Redo,
            (KeyCode::Char(otherwise), _) => {
                if matches!(otherwise, 'f' | 'F' | 't' | 'T' | 'p' | 'P' | 'r') {
                    self.previous_key = Some(otherwise);
                }
                Action::Nothing
            }
            _ => Action::Nothing
        }
        };

        Ok(action)
    }
    fn interpret_command_event(&self, key_event: KeyEvent) -> Result<Action> {
            let action = match key_event.code {
                // Enter will execute different commands for command/find and rfind
                KeyCode::Enter => Action::ExecuteCommand(Command::Find),
                KeyCode::Char(c) => Action::InsertCharAtCursor(c),
                KeyCode::Up => Action::BumpUp,
                KeyCode::Down => Action::BumpDown,
                KeyCode::Backspace => Action::DeleteBefore,
                KeyCode::Left => Action::BumpLeft,
                KeyCode::Right => Action::BumpRight,
                KeyCode::Esc => Action::ChangeMode(Modal::Normal),
                otherwise => notif_bar!(format!("No action bound to {otherwise}");),
            };
            Ok(action)
    }
    fn perform_action(&self, action: Action) -> Result<()> {
        match action {
            Action::Quit => Err(Error::ExitCall),
            Action::BumpUp => todo!(),
            Action::BumpDown => todo!(),
            Action::BumpLeft => todo!(),
            Action::BumpRight => todo!(),
        }
    }
    fn delegate_action(&mut self, action: &Action) -> Result<()> {
        self.cursor.execute_action(action)?;
        self.viewport.execute_action(action)?;
        self.extensions.iter().try_for_each(|e| e.execute_action(action));
        Ok(())
    }
}
