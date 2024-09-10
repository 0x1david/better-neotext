use std::{borrow::Cow, collections::VecDeque, fmt::Debug};

use crate::{
    bars::force_notif_bar_content,
    buffer::TextBuffer,
    cursor::{Cursor, ShadowCursor},
    viewport::ViewPort,
    BaseAction, Command, Component, Error, LineCol, Modal, Pattern, Result,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use tracing::{info, instrument, span, warn, Level};

const JUMP_DIST: usize = 25;

impl<Buff: TextBuffer> Debug for Editor<Buff> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Editor")
    }
}

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

macro_rules! lazy {
    () => {
        Lazy::new()
    };
    ($expr: expr) => {
        Lazy::with_inner($expr)
    };
}

macro_rules! ok_vec{
    ($($expr:expr),* $(,)?) => {
        Ok(vec![$($expr),*])
    }
}

/// Takes an evaluatable base action and evaluates the lazy value inside it while keeping the rest
/// of the attributes unchanged
macro_rules! lazy_eval {
    ($action:expr, $variant:ident, $lazy:expr, $i:expr, $self:expr) => {
        if $lazy.is_evaluated() {
            Cow::Borrowed($action)
        } else {
            Cow::Owned(BaseAction::$variant(lazy!($self.cursor.pos), *$i))
        }
    };
}

impl<Buff: TextBuffer + Debug> Editor<Buff> {
    pub fn new(buff: Buff, without_target: bool) -> Self {
        Self {
            buffer: buff,
            viewport: ViewPort::default(),
            modal: Modal::Normal,
            action_history: Vec::new(),
            action_queue: VecDeque::new(),
            repeat_action: 1,
            previous_key: None,
            cursor: Cursor::default(),
            extensions: Vec::new(),
            shadow_cursor: ShadowCursor { line: 0, col: 0 },
        }
    }
    pub fn run_event_loop(&mut self) -> Result<()> {
        let span = span!(Level::INFO, "event_loop");
        let _guard = span.enter();
        loop {
            let command_buf = self.buffer.get_command_text();
            if !command_buf.is_empty() {
                force_notif_bar_content(command_buf.to_string());
            }
            self.viewport
                .update_viewport(self.buffer.get_normal_text(), &self.cursor)?;
            if let Event::Key(key_event) = event::read()? {
                info!("Interpreting event: {:?}", key_event);
                let action = match self.modal {
                    Modal::Normal => self.interpret_normal_event(key_event),
                    Modal::Insert => self.interpret_insert_event(key_event),
                    Modal::Command | Modal::Find(_) => self.interpret_command_event(key_event),
                    _ => continue,
                }?;

                self.action_history.push(action.clone());
                self.add_to_action_queue(action)?;
                self.consume_action_queue()?;

                self.shadow_cursor.update(self.cursor.pos)
            }
        }
    }
    fn consume_action_queue(&mut self) -> Result<()> {
        info!("Contents of Action Queue: {:?}", self.action_queue);
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

                (KeyCode::Char('W'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    Action::JumpToNextWord
                }
                (KeyCode::Char('w'), KeyModifiers::NONE) => Action::JumpToNextSymbol,
                (KeyCode::Char('B'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    Action::ReverseJumpToNextWord
                }
                (KeyCode::Char('b'), KeyModifiers::NONE) => Action::ReverseJumpToNextSymbol,
                (KeyCode::Char('_'), KeyModifiers::NONE) => Action::JumpSOL,
                (KeyCode::Home, KeyModifiers::NONE) => Action::JumpSOL,
                (KeyCode::Char('$'), KeyModifiers::NONE) => Action::JumpEOL,
                (KeyCode::End, KeyModifiers::NONE) => Action::JumpEOL,
                (KeyCode::Char('g'), KeyModifiers::NONE) => Action::JumpSOF,
                (KeyCode::Char('G'), KeyModifiers::NONE | KeyModifiers::SHIFT) => Action::JumpEOF,

                // Mode Changes
                (KeyCode::Char('i'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Insert),
                (KeyCode::Char('v'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Visual),
                (KeyCode::Char('V'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    Action::ChangeMode(Modal::VisualLine)
                }
                (KeyCode::Char(':'), KeyModifiers::NONE) => Action::ChangeMode(Modal::Command),
                (KeyCode::Char('A'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    Action::InsertModeEOL
                }

                // Text Search
                (KeyCode::Char('/'), KeyModifiers::NONE) => {
                    Action::ChangeMode(Modal::Find(crate::FindDirection::Forwards))
                }
                (KeyCode::Char('?'), KeyModifiers::NONE) => {
                    Action::ChangeMode(Modal::Find(crate::FindDirection::Backwards))
                }

                // Text Manipulation
                (KeyCode::Char('o'), KeyModifiers::NONE) => Action::InsertModeBelow,
                (KeyCode::Char('O'), KeyModifiers::NONE) => Action::InsertModeAbove,
                (KeyCode::Char('X'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    Action::DeleteBeforeCursor
                }
                (KeyCode::Char('x'), KeyModifiers::NONE) => Action::DeleteAtCursor,

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

        Ok(action)
    }
    fn interpret_insert_event(&self, key_event: KeyEvent) -> Result<Action> {
        let action = match key_event.code {
            KeyCode::Char(c) => Action::InsertCharAtCursor(c),
            KeyCode::Enter => Action::InsertNewLine,
            KeyCode::Esc => Action::ChangeMode(Modal::Normal),
            KeyCode::Backspace => Action::DeleteBeforeCursor,
            KeyCode::Left => Action::BumpLeft,
            KeyCode::Right => Action::BumpRight,
            KeyCode::Up => Action::BumpUp,
            KeyCode::Down => Action::BumpDown,
            _ => Action::Nothing,
        };
        Ok(action)
    }
    fn parse_out_command(&self) -> Command {
        let buf = self.buffer.get_command_text();
        info!("Parsing out command: {}", buf);
        let first_ch = buf.chars().next();

        // Parse Command Type
        if let Some(prefix) = first_ch {
            let rest = &buf[1..];
            match prefix {
                '/' => Command::Find(rest.to_string()),
                '?' => Command::Rfind(rest.to_string()),
                // Interpret Command
                _ => match buf {
                    "q" => Command::Exit,
                    _ => Command::None,
                },
            }
        } else {
            Command::None
        }
    }

    fn interpret_command_event(&self, key_event: KeyEvent) -> Result<Action> {
        let action = match key_event.code {
            // Enter will execute different commands based on the parsing of the executecommand/find and rfind
            KeyCode::Enter => {
                let command = self.parse_out_command();
                Action::ExecuteCommand(command)
            }
            KeyCode::Char(c) => Action::InsertCharAtCursor(c),
            KeyCode::Up => Action::BumpUp,
            KeyCode::Down => Action::BumpDown,
            KeyCode::Backspace => Action::DeleteBeforeCursor,
            KeyCode::Left => Action::BumpLeft,
            KeyCode::Right => Action::BumpRight,
            KeyCode::Esc => Action::ChangeMode(Modal::Normal),
            _ => Action::Nothing,
        };
        Ok(action)
    }
    // Decides on how to delegate a given base action
    #[instrument]
    fn perform_action(&mut self, action: BaseAction) -> Result<()> {
        println!("Performing Action: {:?}", action);
        match action {
            BaseAction::MoveUp(_)
            | BaseAction::MoveDown(_)
            | BaseAction::MoveLeft(_)
            | BaseAction::MoveRight(_) => self.delegate_action_bound_checked(&action),
            chm @ BaseAction::ChangeMode(mode) => {
                self.modal = mode;
                self.delegate_action(&chm)
            }
            otherwise => self.delegate_action(&otherwise),
        }
    }

    // Compute the lazy values of BaseActions
    fn compute_lazy_values<'a>(&self, a: &'a BaseAction) -> Cow<'a, BaseAction> {
        match a {
            action @ BaseAction::InsertAt(lazy, i) => lazy_eval!(action, InsertAt, lazy, i, self),
            action @ BaseAction::DeleteAt(lazy, i) => lazy_eval!(action, DeleteAt, lazy, i, self),
            action @ BaseAction::InsertLineAt(lazy, i) => {
                lazy_eval!(action, InsertLineAt, lazy, i, self)
            }
            action @ BaseAction::DeleteLineAt(lazy, i) => {
                lazy_eval!(action, DeleteLineAt, lazy, i, self)
            }
            otherwise => Cow::Borrowed(otherwise),
        }
    }
    #[instrument]
    fn delegate_action(&mut self, action: &BaseAction) -> Result<()> {
        let action = &self.compute_lazy_values(action);

        info!("Delegating Action: {:?}", action);
        self.buffer.execute_action(action)?;
        self.viewport.execute_action(action)?;
        self.cursor.execute_action(action)?;
        self.shadow_cursor.execute_action(action)?;
        self.extensions
            .iter_mut()
            .try_for_each(|e| e.execute_action(action))?;
        Ok(())
    }
    /// Ensures a movement Action fits within bounds, if it doesnt the action is changed to a
    /// bounded version
    #[instrument]
    fn delegate_action_bound_checked(&mut self, action: &BaseAction) -> Result<()> {
        self.shadow_cursor.execute_action(action)?;

        let mut altered = false;

        // Line bound checking
        if self.shadow_cursor.line > self.buffer.max_line() as i64 {
            warn!("Exceeding maximum line, altering action...");
            self.shadow_cursor.line = self.cursor.pos.line as i64;
            let actions = self.resolve_action(Action::JumpEOF)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        } else if self.shadow_cursor.line < 0 {
            warn!("Exceeding minimum line, altering action...");
            self.shadow_cursor.line = self.cursor.pos.line as i64;
            let actions = self.resolve_action(Action::JumpSOF)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        }

        // We need to do these actions in advance if movement command is vertical to have the col
        // bound checking working
        if matches!(action, BaseAction::MoveUp(_) | BaseAction::MoveDown(_)) && !altered {
            warn!("Moving vertically in advance...");
            self.cursor.execute_action(action)?;
            altered = true;
        }

        // Col bound checking
        if self.shadow_cursor.col > self.buffer.max_col(self.shadow_cursor.line as usize) as i64 {
            warn!("Exceeding maximum col, altering action...");
            self.shadow_cursor.col = self.cursor.pos.col as i64;
            let actions = self.resolve_action(Action::JumpEOL)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        } else if self.shadow_cursor.col < 0 {
            warn!("Exceeding minimum col, altering action...");
            self.shadow_cursor.col = self.cursor.pos.col as i64;
            let actions = self.resolve_action(Action::JumpSOL)?;
            for a in actions {
                self.delegate_action(&a)?
            }
            altered = true;
        }

        if !altered {
            info!("executing unaltered action...");
            self.delegate_action(action)?
        };

        Ok(())
    }
    fn resolve_action(&mut self, api_action: Action) -> Result<Vec<BaseAction>> {
        match api_action {
            // No-op and exit actions
            Action::Nothing => ok_vec!(),
            Action::Quit => Err(Error::ExitCall),

            // Basic cursor movements
            Action::BumpUp => ok_vec![BaseAction::MoveUp(1)],
            Action::BumpDown => ok_vec![BaseAction::MoveDown(1)],
            Action::BumpLeft => ok_vec![BaseAction::MoveLeft(1)],
            Action::BumpRight => ok_vec![BaseAction::MoveRight(1)],

            // Larger cursor movements
            Action::JumpUp => ok_vec![BaseAction::MoveUp(JUMP_DIST)],
            Action::JumpDown => ok_vec![BaseAction::MoveDown(JUMP_DIST)],
            Action::JumpSOL => ok_vec![BaseAction::MoveLeft(self.cursor.col())],
            Action::JumpEOL => ok_vec![
                BaseAction::MoveLeft(self.cursor.col()),
                BaseAction::MoveRight(self.buffer.max_col(self.cursor.line()))
            ],
            Action::JumpSOF => ok_vec![BaseAction::MoveUp(self.cursor.line())],
            Action::JumpEOF => ok_vec![
                BaseAction::MoveUp(self.cursor.line()),
                BaseAction::MoveDown(self.buffer.max_line())
            ],

            // Word and symbol navigation
            Action::JumpToNextWord => ok_vec![self.jump_two_boundaries(
                Direction::Forward,
                char::is_whitespace,
                |ch| !ch.is_whitespace(),
            )?],
            Action::JumpToNextSymbol => ok_vec![self.jump_two_boundaries(
                Direction::Forward,
                |ch| ch.is_alphanumeric() || ch == '_',
                |ch| !ch.is_alphanumeric() && ch != '_' && !ch.is_whitespace(),
            )?],
            Action::ReverseJumpToNextWord => ok_vec![self.jump_two_boundaries(
                Direction::Backward,
                char::is_whitespace,
                |ch| !ch.is_whitespace(),
            )?],
            Action::ReverseJumpToNextSymbol => ok_vec![self.jump_two_boundaries(
                Direction::Backward,
                |ch| ch.is_alphanumeric() || ch == '_',
                |ch| !ch.is_alphanumeric() && ch != '_' && !ch.is_whitespace(),
            )?],

            // Find and search actions
            Action::Find(pat) => {
                ok_vec![self.resolve_find(|p, pos| self.find(p, pos), pat)?]
            }
            Action::ReverseFind(pat) => {
                ok_vec![self.resolve_find(|p, pos| self.rfind(p, pos), pat)?]
            }
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
            Action::ChangeMode(mode) => {
                ok_vec![BaseAction::ChangeMode(mode)]
            }
            Action::InsertModeEOL => {
                let dist = self.buffer.max_col(self.cursor.line()) - self.cursor.col();
                ok_vec![
                    BaseAction::MoveRight(dist),
                    BaseAction::ChangeMode(Modal::Insert),
                ]
            }
            Action::InsertModeBelow => ok_vec![
                BaseAction::MoveDown(1),
                BaseAction::ChangeMode(Modal::Insert),
            ],
            Action::InsertModeAbove => {
                ok_vec![BaseAction::MoveUp(1), BaseAction::ChangeMode(Modal::Insert),]
            }

            // Edit actions
            Action::Save => ok_vec![BaseAction::Save],
            Action::Yank => ok_vec![BaseAction::Yank],
            Action::Redo => ok_vec![BaseAction::Redo(1)],
            Action::DeleteAtCursor => ok_vec![BaseAction::DeleteAt(lazy!(), 1),],
            Action::Replace(char) => {
                ok_vec![
                    BaseAction::DeleteAt(lazy!(), 1),
                    BaseAction::InsertAt(lazy!(), char),
                ]
            }
            Action::DeleteBeforeCursor => {
                ok_vec![BaseAction::MoveLeft(1), BaseAction::DeleteAt(lazy!(), 1)]
            }
            Action::Undo(steps) => ok_vec![BaseAction::Undo(steps.into())],
            Action::InsertCharAtCursor(ch) => {
                ok_vec![BaseAction::InsertAt(lazy!(), ch), BaseAction::MoveRight(1)]
            }

            // Paste actions
            Action::Paste(reg) => ok_vec![BaseAction::Paste(reg, 1)],
            Action::PasteAbove(reg) => ok_vec![BaseAction::Paste(reg, 1)],
            Action::PasteNewline(reg) => {
                ok_vec![BaseAction::MoveDown(1), BaseAction::Paste(reg, 1)]
            }

            // Miscellaneous actions
            Action::OpenFile => ok_vec![BaseAction::OpenFile],
            Action::InsertNewLine => ok_vec![
                BaseAction::InsertLineAt(lazy!(), 1),
                BaseAction::MoveDown(1)
            ],
            Action::FetchFromHistory => ok_vec![BaseAction::FetchFromHistory],
            Action::ExecuteCommand(c) => self.resolve_command_action(c),
        }
    }
    fn resolve_command_action(&self, c: Command) -> Result<Vec<BaseAction>> {
        match c {
            Command::Exit => Err(Error::ExitCall),
            Command::None => ok_vec![BaseAction::ChangeMode(Modal::Normal)],
            Command::Find(s) => {
                let lc = self.find(s, self.cursor.last_text_mode_pos);
                info!("Found match for find on {:?}", lc);

                match lc {
                    Err(Error::PatternNotFound) => ok_vec!(BaseAction::ChangeMode(Modal::Normal)),
                    Ok(target) => self.calculate_jump_actions(target),
                    Err(e) => Err(e),
                }
            }
            Command::Rfind(s) => {
                let lc = self.rfind(s, self.cursor.last_text_mode_pos);
                info!("Found match for rfind on {:?}", lc);

                match lc {
                    Err(Error::PatternNotFound) => ok_vec!(BaseAction::ChangeMode(Modal::Normal)),
                    Ok(target) => self.calculate_jump_actions(target),
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn calculate_jump_actions(&self, target: LineCol) -> Result<Vec<BaseAction>> {
        let mut action_vec = vec![];
        action_vec.push(BaseAction::ChangeMode(Modal::Normal));
        let from = self.cursor.last_text_mode_pos;

        action_vec.push(BaseAction::MoveLeft(self.cursor.text_mode_col()));

        match from.line.cmp(&target.line) {
            std::cmp::Ordering::Less => {
                action_vec.push(BaseAction::MoveDown(target.line - from.line))
            }
            std::cmp::Ordering::Greater => {
                action_vec.push(BaseAction::MoveUp(from.line - target.line))
            }
            std::cmp::Ordering::Equal => (),
        };

        action_vec.push(BaseAction::MoveRight(target.col));
        info!(
            "Movement needs to be done to reach target: {:?}",
            action_vec
        );

        Ok(action_vec)
    }
    /// Resolves the input action and adds corresponding BaseActions to the queue
    #[instrument]
    fn add_to_action_queue(&mut self, api_action: Action) -> Result<()> {
        let mut base_actions = self.resolve_action(api_action)?;

        // If repeatable
        if base_actions.len() == 1 && self.repeat_action != 1 {
            let a = base_actions
                .pop()
                .expect("Checked for len one line prior")
                .repeat(self.repeat_action);
            self.action_queue.push_back(a);
        }

        for action in base_actions {
            self.action_queue.push_back(action)
        }
        Ok(())
    }

    fn resolve_find<F, P>(&self, find_fn: F, pattern: P) -> Result<BaseAction>
    where
        F: Fn(P, LineCol) -> Result<LineCol>,
        P: Pattern,
    {
        // TODO: After Regex Implementation this part of the code will decide and convert
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
        let mut pos = self.cursor.last_text_mode_pos;

        // Avoid getting stuck if jump destination is directly on cursor
        if self.buffer.max_normal_col(pos.line) > pos.col {
            pos.col += 1;
        }

        let dest = match direction {
            Direction::Forward => {
                let dest = self.find(&first_boundary, pos);
                if let Err(Error::PatternNotFound) = dest {
                    warn!("First Destination not found");
                    return Ok(BaseAction::Nothing);
                };
                let dest = dest?;

                info!("First Destination found{:?}", &dest);
                let dest = self.find(&second_boundary, dest);
                if let Err(Error::PatternNotFound) = dest {
                    warn!("Second Destination not found");
                    return Ok(BaseAction::Nothing);
                };
                info!("Second Destination found{:?}", &dest);
                dest?
            }
            Direction::Backward => {
                let dest = self.rfind(&first_boundary, pos)?;
                self.rfind(&second_boundary, dest)?
            }
        };

        Ok(BaseAction::SetCursor(dest))
    }

    /// Searches for a query string in the buffer, starting from a given position.
    ///
    /// # Arguments
    ///
    /// * `query` - The string to search for.
    /// * `at` - The position (line and column) to start the search from.
    ///
    /// # Returns
    ///
    /// * `Ok(LineCol)` - The position (line and column) where the query was found.
    /// * `Err(BufferError::PatternNotFound)` - If the query string is not found in the buffer.
    ///
    /// # Behavior
    ///
    /// The search starts at the given position and continues to the end of the buffer.
    /// It searches the remainder of the starting line, then subsequent lines in their entirety.
    /// The search is case-sensitive and returns the position of the first occurrence found.
    ///
    /// # Examples
    ///
    /// ```
    /// let buffer = // ... initialize buffer ...
    /// let result = buffer.find("example", LineCol{line: 1, col: 5});
    /// assert_eq!(result, Ok(LineCol{line: 2, col: 10})); // Found on line 2, column 10
    /// ```
    fn find(&self, query: impl Pattern, at: LineCol) -> Result<LineCol> {
        let buf = &self.buffer.get_buffer_window(Some(at), None)?;
        info!("{:?}", buf);
        query
            .find_pattern(buf)
            .ok_or(Error::PatternNotFound)
            .map(|target| LineCol {
                line: target.line + at.line,
                col: if target.line == 0 {
                    target.col + at.col
                } else {
                    target.col
                },
            })
    }

    /// Searches backwards for a query string in the buffer, ending at a given position.
    ///
    /// # Arguments
    ///
    /// * `query` - The string to search for.
    /// * `at` - The position (line and column) to start the reverse search from.
    ///
    /// # Returns
    ///
    /// * `Ok(LineCol)` - The position (line and column) where the query was found.
    /// * `Err(BufferError::PatternNotFound)` - If the query string is not found in the buffer.
    ///
    /// # Behavior
    ///
    /// The search starts at the given position and continues backwards to the beginning of the buffer.
    /// It first searches the portion of the starting line from the given position to its start,
    /// then searches previous lines in their entirety from end to start.
    /// The search is case-sensitive and returns the position of the last occurrence found
    /// (i.e., the first occurrence when searching backwards).
    ///
    /// # Examples
    ///
    /// ```
    /// let buffer = // ... initialize buffer ...
    /// let result = buffer.rfind("example", LineCol{line: 2, col: 15});
    /// assert_eq!(result, Ok(LineCol{line: 1, col: 5})); // Found on line 1, column 5
    /// ```
    fn rfind(&self, query: impl Pattern, at: LineCol) -> Result<LineCol> {
        let buf = &self.buffer.get_buffer_window(None, Some(at))?;
        query
            .rfind_pattern(buf)
            .ok_or(Error::PatternNotFound)
            .map(|v| LineCol {
                line: v.line,
                col: v.col,
            })
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
    InsertNewLine,
    InsertModeBelow,
    InsertModeAbove,
    DeleteBeforeCursor,
    DeleteAtCursor,

    // Clipboard Operations
    Yank,
    Paste(char),
    PasteNewline(char),
    PasteAbove(char),

    // History Operations
    FetchFromHistory,

    // Command Execution
    ExecuteCommand(Command),

    // Undo/Redo
    Undo(u8),
    Redo,

    // Misc
    OpenFile,

    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lazy<T> {
    inner: Option<T>,
}
impl<T> Lazy<T> {
    /// Create Type Inferred Lazy Struct
    pub fn new() -> Self {
        Lazy { inner: None }
    }
    /// Returns the inner evaluated value if there is one
    pub fn into_inner(self) -> Option<T> {
        self.inner
    }
    /// Initializes a Lazy Type with an already initialized value.
    pub fn with_inner(v: T) -> Self {
        Lazy { inner: Some(v) }
    }
    /// Set the inner value Of Lazy value
    pub fn set_inner(&mut self, v: T) {
        self.inner = Some(v)
    }
    /// True if the lazy already contains an inner value
    pub fn is_evaluated(&self) -> bool {
        self.inner.is_some()
    }
}

impl<T: Clone> Lazy<T> {
    // Clones the inner type T, panicking if called on an unevaluated lazy
    pub fn clone_inner(&self) -> T {
        self.inner.clone().unwrap()
    }
}

impl<T> Default for Lazy<T> {
    fn default() -> Self {
        Self::new()
    }
}

mod test {
    use super::*;
    use crate::buffer::VecBuffer;
    use crate::LineCol;

    #[test]
    fn test_jump_two_boundaries() {
        // Create a VecBuffer with test content
        let content = vec![
            "Hello world! This is a test.".to_string(),
            "Multiple   spaces   between words.".to_string(),
            "Symbols: @#$% and _underscores_".to_string(),
        ];
        let buffer = VecBuffer::new(content);

        let mut editor = Editor::new(buffer, false);

        // Test forward word jump
        let result = editor.jump_two_boundaries(Direction::Forward, char::is_whitespace, |ch| {
            !ch.is_whitespace()
        });
        assert_eq!(
            result.unwrap(),
            BaseAction::SetCursor(LineCol { line: 0, col: 6 })
        );

        // Test backward word jump
        editor.cursor.pos = LineCol { line: 0, col: 20 };
        let result = editor.jump_two_boundaries(Direction::Backward, char::is_whitespace, |ch| {
            !ch.is_whitespace()
        });
        assert_eq!(
            result.unwrap(),
            BaseAction::SetCursor(LineCol { line: 0, col: 13 })
        );

        // Test forward symbol jump
        editor.cursor.pos = LineCol { line: 2, col: 0 };
        let result = editor.jump_two_boundaries(
            Direction::Forward,
            |ch| ch.is_alphanumeric() || ch == '_',
            |ch| !ch.is_alphanumeric() && ch != '_' && !ch.is_whitespace(),
        );
        assert_eq!(
            result.unwrap(),
            BaseAction::SetCursor(LineCol { line: 2, col: 8 })
        );

        // Test backward symbol jump
        editor.cursor.pos = LineCol { line: 2, col: 20 };
        let result = editor.jump_two_boundaries(
            Direction::Backward,
            |ch| ch.is_alphanumeric() || ch == '_',
            |ch| !ch.is_alphanumeric() && ch != '_' && !ch.is_whitespace(),
        );
        assert_eq!(
            result.unwrap(),
            BaseAction::SetCursor(LineCol { line: 2, col: 13 })
        );

        // Test jump at end of buffer
        editor.cursor.pos = LineCol { line: 2, col: 30 };
        let result = editor.jump_two_boundaries(Direction::Forward, char::is_whitespace, |ch| {
            !ch.is_whitespace()
        });
        assert_eq!(result.unwrap(), BaseAction::Nothing);

        // Test jump at beginning of buffer
        editor.cursor.pos = LineCol { line: 0, col: 0 };
        let result = editor.jump_two_boundaries(Direction::Backward, char::is_whitespace, |ch| {
            !ch.is_whitespace()
        });
        assert_eq!(result.unwrap(), BaseAction::Nothing);
    }
}
