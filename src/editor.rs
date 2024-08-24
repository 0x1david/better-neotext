use crate::{buffer::{TextBuffer, VecBuffer}, cursor::{Cursor, ShadowCursor}, viewport::ViewPort, Action, Command, Component, Error, FindMode, LineCol, Modal, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};


pub struct Editor<Buff: TextBuffer> {
    buffer: Buff,
    viewport: ViewPort,
    modal: Modal,
    action_history: Vec<Action>,
    repeat_action: usize,
    previous_key: Option<char>,
    cursor: Cursor,
    shadow_cursor: ShadowCursor,
    extensions: Vec<Box<dyn Component>>,
}

impl<Buff: TextBuffer> Editor<Buff> {
    pub fn new(buff: Buff, without_target: bool) -> Self {
        Self {
            buffer: buff,
            viewport: ViewPort::default(),
            modal: Modal::Normal,
            action_history: vec![],
            repeat_action: 1,
            previous_key: None,
            cursor: Cursor::default(),
            extensions: vec![],
            shadow_cursor: ShadowCursor {line: 0, col: 0}
        }
    }
    pub fn run_event_loop(&mut self) -> Result<()>{
        loop {
            self.viewport.update_viewport(self.buffer.get_entire_text(), &self.cursor)?;
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
                _ => Action::Nothing
            };
            Ok(action)
    }
    fn perform_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => Err(Error::ExitCall),
            Action::BumpUp | 
            Action::BumpDown | 
            Action::BumpLeft | 
            Action::BumpRight |
            Action::JumpUp |
            Action::JumpDown
            => self.delegate_action_checked(&action),
            _ => Ok(())
        }
    }
    fn delegate_action(&mut self, action: &Action) -> Result<()> {
        self.cursor.execute_action(action)?;
        self.viewport.execute_action(action)?;
        self.extensions.iter_mut().try_for_each(|e| e.execute_action(action))?;
        Ok(())
    }
    /// Ensures a movement Action fits within bounds, if it doesnt the action is changed to a
    /// bounded version
    fn delegate_action_checked(&mut self, action: &Action) -> Result<()> {
        self.shadow_cursor.execute_action(action)?;

        let mut altered = false;

        // Line bound checking
        if self.shadow_cursor.line > self.buffer.max_line() as i64 {
            self.delegate_action(&Action::JumpEOF)?;
            self.shadow_cursor.update(&self.cursor.pos);
            altered = true;
        } else if self.shadow_cursor.line < 0 {
            self.delegate_action(&Action::JumpSOF)?;
            altered = true;
            self.shadow_cursor.line = 0;
        }

        // Col bound checking
        if self.shadow_cursor.col > self.buffer.max_col(self.shadow_cursor.line as usize) as i64 {
            self.delegate_action(&Action::JumpEOL)?;
            altered = true;
            self.shadow_cursor.update(&self.cursor.pos);
        } else if self.shadow_cursor.col < 0 {
            self.delegate_action(&Action::JumpSOL)?;
            altered = true;
            self.shadow_cursor.col = 0;
        }
        
        if !altered {
            self.delegate_action(action)?
        };

        Ok(())
    }
}
