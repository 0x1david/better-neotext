use std::{io::stdout, process::exit};

use crate::{buffer::TextBuffer, viewport::ViewPort, Action, Command, Error, FindMode, Modal, Result};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyModifiers}, execute, terminal};

pub struct Editor<Buff: TextBuffer> {
    buffer: Buff,
    viewport: ViewPort,
    modal: Modal,
    action_history: Vec<Action>,
    repeat_action: usize,
}

impl<Buff: TextBuffer> Editor<Buff> {
    fn run_event_loop(&self) -> Result<()>{
        loop {
            self.viewport.update_viewport();
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
   let action = match (key_event.code, key_event.modifiers) {
        // Cursor Movement
        (KeyCode::Char('k'), KeyModifiers::NONE) => Action::BumpUp,
        (KeyCode::Char('j'), KeyModifiers::NONE) => Action::BumpDown,
        (KeyCode::Char('h'), KeyModifiers::NONE) => Action::BumpLeft,
        (KeyCode::Char('l'), KeyModifiers::NONE) => Action::BumpRight,
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => Action::JumpUp,
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => Action::JumpDown,

        (KeyCode::Char('f'), KeyModifiers::NONE) => Action::JumpLetter(' '), // Placeholder, actual char would be read next
        (KeyCode::Char('F'), KeyModifiers::NONE) => Action::ReverseJumpLetter(' '), // Placeholder
        (KeyCode::Char('W'), KeyModifiers::NONE) => Action::JumpToNextWord,
        (KeyCode::Char('w'), KeyModifiers::NONE) => Action::JumpToNextSymbol,
        (KeyCode::Char('B'), KeyModifiers::NONE) => Action::ReverseJumpToNextWord,
        (KeyCode::Char('b'), KeyModifiers::NONE) => Action::ReverseJumpToNextSymbol,
        (KeyCode::Char('_'), KeyModifiers::NONE) => Action::JumpSOL,
        (KeyCode::Home, _) => Action::JumpSOL,
        (KeyCode::Char('$'), KeyModifiers::NONE) => Action::JumpEOL,
        (KeyCode::End, _) => Action::JumpEOL,
        (KeyCode::Char('g'), KeyModifiers::NONE) => Action::JumpSOF, // Placeholder
        (KeyCode::Char('G'), KeyModifiers::NONE) => Action::JumpEOF,

        // Mode Changes
        (KeyCode::Char('v'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Visual),
        (KeyCode::Char('V'), KeyModifiers::NONE) => Action::ChangeMode(Modal::VisualLine),
        (KeyCode::Char(':'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Command),
        (KeyCode::Char('A'), KeyModifiers::NONE) => Action::InsertModeEOL,

        // Text Search
        (KeyCode::Char('/'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Find(FindMode::Forwards)),
        (KeyCode::Char('?'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Find(FindMode::Backwards)),
        (KeyCode::Char('f'), KeyModifiers::NONE) => Action::FindChar(' '), // PlaceholderA
        (KeyCode::Char('F'), KeyModifiers::NONE) => Action::ReverseFindChar(' '), // Placeholder

        // Text Manipulation
        (KeyCode::Char('r'), KeyModifiers::NONE) => Action::Replace(' '), // Placeholder
        (KeyCode::Char('o'), KeyModifiers::NONE) => Action::InsertNewline,
        (KeyCode::Char('O'), KeyModifiers::NONE) => Action::InsertBelow,
        (KeyCode::Char('X'), KeyModifiers::NONE) => Action::DeleteBefore,
        (KeyCode::Char('x'), KeyModifiers::NONE) => Action::DeleteUnder,

        // Clipboard Operations
        (KeyCode::Char('p'), KeyModifiers::NONE) => Action::Paste('0'),
        (KeyCode::Char('P'), KeyModifiers::NONE) => Action::PasteAbove('0'),

        // Undo/Redo
        (KeyCode::Char('u'), KeyModifiers::NONE) => Action::Undo(1),
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => Action::Redo,
        otherwise => notif_bar!(format!("No action bound to {otherwise}");),
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
}
