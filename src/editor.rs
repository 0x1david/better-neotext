use std::collections::VecDeque;

use crate::{
    buffer::TextBuffer,
    cursor::{Cursor, ShadowCursor},
    viewport::ViewPort,
    BaseAction, Command, Component, Error, FindMode, LineCol, Modal, Pattern, Result,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

const JUMP_DIST: usize = 25;

pub struct Editor<Buff: TextBuffer> {
    buffer: Buff,
    viewport: ViewPort,
    modal: Modal,
    action_history: Vec<Action>,
    action_queue: VecDeque<BaseAction>,
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
            action_history: Vec::new(),
            action_queue: VecDeque::new(),
            repeat_action: 0,
            previous_key: None,
            cursor: Cursor::default(),
            extensions: Vec::new(),
            shadow_cursor: ShadowCursor { line: 0, col: 0 },
        }
    }
    pub fn run_event_loop(&mut self) -> Result<()> {
        loop {
            self.viewport
                .update_viewport(self.buffer.get_entire_text(), &self.cursor)?;
            if let Event::Key(key_event) = event::read()? {
                let action = match self.modal {
                    Modal::Normal => self.interpret_normal_event(key_event),
                    Modal::Insert => self.interpret_insert_event(key_event),
                    Modal::Command => self.interpret_command_event(key_event),
                    _ => continue,
                }?;

                self.action_history.push(action.clone());
                self.add_to_action_queue(action)?;
                self.consume_action_queue()?;

                self.shadow_cursor.update(self.cursor.pos)
            }
            return Ok(());
        }
    }
    fn consume_action_queue(&mut self) -> Result<()> {
        let actions: Vec<_> = self.action_queue.drain(..).collect();
        for action in actions {
            self.perform_action(action)?;
        }
        Ok(())
    }
    fn interpret_normal_event(&mut self, key_event: KeyEvent) -> Result<Action> {
        let action = if let Some(prev) = self.previous_key.take() {
            match (prev, key_event.code) {
                ('t', KeyCode::Char(c)) => Action::FindChar(c),
                ('T', KeyCode::Char(c)) => Action::ReverseFindChar(c),
                ('f', KeyCode::Char(c)) => Action::FindChar(c),
                ('F', KeyCode::Char(c)) => Action::ReverseFindChar(c),
                ('r', KeyCode::Char(c)) => Action::Replace(c),
                ('p', KeyCode::Char(c)) => Action::Paste(c),
                ('P', KeyCode::Char(c)) => Action::PasteAbove(c),
                _ => Action::Nothing,
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
                (KeyCode::Char('/'), KeyModifiers::NONE) => {
                    Action::ChangeMode(Modal::Find(FindMode::Forwards))
                }
                (KeyCode::Char('?'), KeyModifiers::NONE) => {
                    Action::ChangeMode(Modal::Find(FindMode::Backwards))
                }

                // Text Manipulation
                (KeyCode::Char('o'), KeyModifiers::NONE) => Action::InsertModeBelow,
                (KeyCode::Char('O'), KeyModifiers::NONE) => Action::InsertModeAbove,
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
                _ => Action::Nothing,
            }
        };

        println!("Translated Action: {:?}", action);
        Ok(action)
    }
    fn interpret_insert_event(&self, key_event: KeyEvent) -> Result<Action> {
        todo!()
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
            _ => Action::Nothing,
        };
        Ok(action)
    }
    // Decides on how to delegate a given base action
    fn perform_action(&mut self, action: BaseAction) -> Result<()> {
        println!("Performing Action: {:?}", action);
        match action {
            BaseAction::MoveUp(_)
            | BaseAction::MoveDown(_)
            | BaseAction::MoveLeft(_)
            | BaseAction::MoveRight(_)
            | BaseAction::SetCursor(_) => self.delegate_action_bound_checked(&action),
            _ => Ok(()),
        }
    }
    fn delegate_action(&mut self, action: &BaseAction) -> Result<()> {
        println!("Delegating Action: {:?}", action);
        self.cursor.execute_action(action)?;
        self.viewport.execute_action(action)?;
        self.shadow_cursor.execute_action(action)?;
        self.extensions
            .iter_mut()
            .try_for_each(|e| e.execute_action(action))?;
        Ok(())
    }
    /// Ensures a movement Action fits within bounds, if it doesnt the action is changed to a
    /// bounded version
    fn delegate_action_bound_checked(&mut self, action: &BaseAction) -> Result<()> {
        self.shadow_cursor.execute_action(action)?;

        let mut altered = false;

        // Line bound checking
        if self.shadow_cursor.line > self.buffer.max_line() as i64 {
            self.shadow_cursor.line = self.cursor.pos.line as i64;
            let actions = self.resolve_action(Action::JumpEOF)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        } else if self.shadow_cursor.line < 0 {
            self.shadow_cursor.line = self.cursor.pos.line as i64;
            let actions = self.resolve_action(Action::JumpSOF)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        }

        // Col bound checking
        if self.shadow_cursor.col > self.buffer.max_col(self.shadow_cursor.line as usize) as i64 {
            self.shadow_cursor.col = self.cursor.pos.col as i64;
            let actions = self.resolve_action(Action::JumpEOL)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        } else if self.shadow_cursor.col < 0 {
            self.shadow_cursor.col = self.cursor.pos.col as i64;
            let actions = self.resolve_action(Action::JumpSOL)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        }

        if !altered {
            self.delegate_action(action)?
        };

        Ok(())
    }
    fn resolve_action(&self, api_action: Action) -> Result<Vec<BaseAction>> {
        match api_action {
            // No-op and exit actions
            Action::Nothing => Ok(vec![]),
            Action::Quit => Err(Error::ExitCall),

            // Basic cursor movements
            Action::BumpUp => Ok(vec![BaseAction::MoveUp(1)]),
            Action::BumpDown => Ok(vec![BaseAction::MoveDown(1)]),
            Action::BumpLeft => Ok(vec![BaseAction::MoveLeft(1)]),
            Action::BumpRight => Ok(vec![BaseAction::MoveRight(1)]),

            // Larger cursor movements
            Action::JumpUp => Ok(vec![BaseAction::MoveUp(JUMP_DIST)]),
            Action::JumpDown => Ok(vec![BaseAction::MoveDown(JUMP_DIST)]),
            Action::JumpSOL => Ok(vec![BaseAction::MoveLeft(self.cursor.col())]),
            Action::JumpEOL => Ok(vec![BaseAction::MoveRight(
                self.buffer.max_col(self.cursor.line()) - self.cursor.col(),
            )]),
            Action::JumpSOF => Ok(vec![BaseAction::MoveUp(self.cursor.line())]),
            Action::JumpEOF => Ok(vec![BaseAction::MoveDown(self.buffer.max_line())]),

            // Word and symbol navigation
            Action::JumpToNextWord => Ok(vec![self.jump_two_boundaries(
                Direction::Forward,
                char::is_whitespace,
                |ch| !ch.is_whitespace(),
            )?]),
            Action::JumpToNextSymbol => Ok(vec![self.jump_two_boundaries(
                Direction::Forward,
                |ch| !ch.is_whitespace(),
                |ch| !ch.is_whitespace(),
            )?]),
            Action::ReverseJumpToNextWord => Ok(vec![self.jump_two_boundaries(
                Direction::Backward,
                char::is_whitespace,
                |ch| !ch.is_whitespace(),
            )?]),
            Action::ReverseJumpToNextSymbol => Ok(vec![self.jump_two_boundaries(
                Direction::Backward,
                |ch| !ch.is_whitespace(),
                |ch| !ch.is_whitespace(),
            )?]),

            // Find and search actions
            Action::Find(pat) => Ok(vec![
                self.resolve_find(|p, pos| self.buffer.find(p, pos), pat)?
            ]),
            Action::ReverseFind(pat) => Ok(vec![
                self.resolve_find(|p, pos| self.buffer.rfind(p, pos), pat)?
            ]),
            Action::FindChar(ch) => self.resolve_action(Action::Find(ch.to_string())),
            Action::ReverseFindChar(ch) => self.resolve_action(Action::ReverseFind(ch.to_string())),
            Action::ToChar(ch) => {
                let mut actions = self.resolve_action(Action::FindChar(ch))?;
                actions.push(BaseAction::MoveLeft(1));
                Ok(actions)
            }
            Action::ReverseToChar(ch) => {
                let mut actions = self.resolve_action(Action::ReverseFindChar(ch))?;
                actions.push(BaseAction::MoveRight(1));
                Ok(actions)
            }

            // Mode change actions
            Action::ChangeMode(mode) => Ok(vec![BaseAction::ChangeMode(mode)]),
            Action::InsertModeEOL => {
                let dist = self.buffer.max_col(self.cursor.line()) - self.cursor.col();
                Ok(vec![
                    BaseAction::MoveRight(dist),
                    BaseAction::ChangeMode(Modal::Insert),
                ])
            }
            Action::InsertModeBelow => Ok(vec![
                BaseAction::MoveDown(1),
                BaseAction::ChangeMode(Modal::Insert),
            ]),
            Action::InsertModeAbove => Ok(vec![
                BaseAction::MoveUp(1),
                BaseAction::ChangeMode(Modal::Insert),
            ]),

            // Edit actions
            Action::Save => Ok(vec![BaseAction::Save]),
            Action::Yank => Ok(vec![BaseAction::Yank]),
            Action::Redo => Ok(vec![BaseAction::Redo]),
            Action::DeleteUnder => Ok(vec![
                BaseAction::MoveDown(1),
                BaseAction::DeleteCurrentLine,
                BaseAction::MoveUp(1),
            ]),
            Action::Replace(char) => Ok(vec![
                BaseAction::DeleteUnderCursor,
                BaseAction::InsertUnderCursor(char),
            ]),
            Action::DeleteBefore => {
                Ok(vec![BaseAction::MoveLeft(1), BaseAction::DeleteUnderCursor])
            }
            Action::Undo(steps) => Ok(vec![BaseAction::Undo(steps)]),
            Action::InsertCharAtCursor(ch) => Ok(vec![BaseAction::InsertUnderCursor(ch)]),

            // Paste actions
            Action::Paste(reg) => Ok(vec![BaseAction::Paste(reg)]),
            Action::PasteAbove(reg) => Ok(vec![BaseAction::Paste(reg)]),
            Action::PasteNewline(reg) => Ok(vec![BaseAction::MoveDown(1), BaseAction::Paste(reg)]),

            // Miscellaneous actions
            Action::OpenFile => Ok(vec![BaseAction::OpenFile]),
            Action::InsertTab => Ok(vec![]), // Currently not implemented
            Action::ExecuteCommand(_command) => unimplemented!(),
            Action::FetchFromHistory(idx) => Ok(vec![BaseAction::FetchFromHistory(idx)]),
        }
    }
    /// Resolves the input action and adds corresponding BaseActions to the queue
    fn add_to_action_queue(&mut self, api_action: Action) -> Result<()> {
        let base_actions = self.resolve_action(api_action)?;
        for action in base_actions {
            self.add_action(action);
        }
        Ok(())
    }

    fn resolve_find<F, P>(&self, find_fn: F, pattern: P) -> Result<BaseAction>
    where
        F: Fn(P, LineCol) -> Result<LineCol>,
        P: Pattern,
    {
        // After Regex Implementation this part of the code will decide and convert
        // pattern to regex if needed
        let pos = self.cursor.pos;
        let dest = match find_fn(pattern, pos) {
            Err(Error::PatternNotFound) => pos,
            Ok(dest) => dest,
            e => e?,
        };
        Ok(BaseAction::SetCursor(dest))
    }
    fn jump_two_boundaries<F1, F2>(
        &self,
        direction: Direction,
        first_boundary: F1,
        second_boundary: F2,
    ) -> Result<BaseAction>
    where
        F1: Fn(char) -> bool,
        F2: Fn(char) -> bool,
    {
        let mut pos = self.cursor.pos;

        // Avoid getting stuck if jump destination is directly on cursor
        if self.buffer.max_col(pos.line) > pos.col {
            pos.col += 1;
        }

        let dest = match direction {
            Direction::Forward => {
                let dest = self.buffer.find(&first_boundary, pos)?;
                self.buffer.find(&second_boundary, dest)?
            }
            Direction::Backward => {
                let dest = self.buffer.rfind(&first_boundary, pos)?;
                self.buffer.rfind(&second_boundary, dest)?
            }
        };

        Ok(BaseAction::SetCursor(dest))
    }
    #[inline]
    fn add_action(&mut self, a: BaseAction) {
        self.action_queue.push_back(a)
    }
}

enum Direction {
    Forward,
    Backward,
}

#[derive(Clone, Debug)]
enum Action {
    Quit,
    Save,

    // Cursor Movement
    BumpUp,
    BumpDown,
    BumpLeft,
    BumpRight,
    JumpUp,
    JumpDown,
    JumpToNextWord,
    JumpToNextSymbol,
    ReverseJumpToNextWord,
    ReverseJumpToNextSymbol,
    JumpSOL,
    JumpEOL,
    JumpSOF,
    JumpEOF,

    // Mode Changes
    ChangeMode(Modal),
    InsertModeEOL,

    // Text Search
    Find(String),
    ReverseFind(String),
    FindChar(char),
    ReverseFindChar(char),
    ReverseToChar(char),
    ToChar(char),

    // Insertions

    // Text Manipulation
    Replace(char),
    InsertCharAtCursor(char),
    InsertModeBelow,
    InsertModeAbove,
    InsertTab,
    DeleteBefore,
    DeleteUnder,

    // Clipboard Operations
    Yank,
    Paste(char),
    PasteNewline(char),
    PasteAbove(char),

    // History Operations
    FetchFromHistory(u8),

    // Command Execution
    ExecuteCommand(Command),

    // Undo/Redo
    Undo(u8),
    Redo,

    // Misc
    OpenFile,

    Nothing,
}
