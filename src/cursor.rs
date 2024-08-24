use std::{cmp::Ordering, fmt::Display};

use crate::{Component, Modal, Action, LineCol};

const JUMP_DIST: usize = 25;


#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub start: LineCol,
    pub end: LineCol,
}

impl Selection {
    pub const fn line_is_in_selection(&self, line: usize) -> bool {
        self.start.line < line && self.end.line > line
    }
    pub fn normalized(mut self) -> Self {
        if self.end < self.start {
            std::mem::swap(&mut self.end, &mut self.start);
        };
        self
    }
}


/// The overarching cursor struct
#[derive(Clone, Debug)]
pub struct Cursor {
    pub pos: LineCol,
    pub previous_pos: LineCol,
    pos_initial: LineCol,
    plane: CursorPlane,
    pub last_text_mode_pos: LineCol,
}

pub struct ShadowCursor {pub line: i64, pub col: i64}

impl From<&LineCol> for ShadowCursor {
    fn from(value: &LineCol) -> Self {
        Self {
            line:  value.line as i64,
            col:  value.col as i64,
        }
    }
}
impl ShadowCursor {
    pub fn update(&mut self, lc: &LineCol) {
        self.line = lc.line as i64;
        self.col = lc.col as i64;
    }
}

impl Component for ShadowCursor {
    fn execute_action(&mut self, a: &Action) -> crate::Result<()> {
        match a {
            Action::BumpUp => self.line += 1,
            Action::BumpDown => self.line -= 1,
            Action::BumpLeft => self.col -= 1,
            Action::BumpRight=> self.col += 1,
            Action::JumpUp => self.line += JUMP_DIST as i64,
            Action::JumpDown => self.line -= JUMP_DIST as i64,
            Action::SetCursor(lc) => {
                self.update(lc)
            }
            _ => (),
        };
        Ok(())
    }
}

impl Component for Cursor {
    fn execute_action(&mut self, a: &crate::Action) -> crate::Result<()> {
        match a {
            Action::BumpUp => self.bump_up(),
            Action::BumpDown => self.bump_down(),
            Action::BumpLeft => self.bump_left(),
            Action::BumpRight=> self.bump_right(),
            Action::JumpUp => self.jump_up(JUMP_DIST),
            Action::JumpDown => self.jump_down(JUMP_DIST),
            Action::JumpSOL => self.set_col(0),
            Action::JumpSOF => self.set_line(0),
            Action::SetCursor(lc) => self.go(lc),
            _ => (),
        };
        Ok(())
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            pos: LineCol::default(),
            previous_pos: LineCol::default(),
            pos_initial: LineCol::default(),
            plane: CursorPlane::Text,
            last_text_mode_pos: LineCol::default(),
        }
    }
}

impl Cursor {
    #[inline]
    pub fn go(&mut self, to: &LineCol) {
        self.previous_pos = self.pos;
        self.pos = to.clone();
    }
    #[inline]
    pub const fn line(&self) -> usize {
        self.pos.line
    }

    #[inline]
    pub fn set_line(&mut self, new: usize) {
        self.previous_pos = self.pos;
        self.pos.line = new;
    }

    #[inline]
    pub const fn col(&self) -> usize {
        self.pos.col
    }

    #[inline]
    pub fn set_col(&mut self, new: usize) {
        self.previous_pos = self.pos;
        self.pos.col = new;
    }

    /// Moves the cursor one position to the left, if there's left to go to, otherwise remains in
    /// place.
    #[inline]
    pub fn bump_left(&mut self) {
        self.previous_pos = self.pos;
        if self.col() != 0 {
            self.pos.col -= 1;
        }
    }

    /// Moves the cursor one position to the right, if there's right to go to, otherwise remains in
    /// place.
    #[inline]
    pub fn bump_right(&mut self) {
        self.previous_pos = self.pos;
        self.pos.col += 1;
    }

    /// Moves the cursor one position up, if there's upper line to go to, otherwise remains in
    /// place.
    #[inline]
    fn bump_up(&mut self) {
        if self.line() != 0 {
            self.pos.line -= 1;
        }
    }

    /// Moves the cursor one position down, if there's lower line to go to, otherwise remains in
    /// place.
    #[inline]
    fn bump_down(&mut self) {
        self.previous_pos = self.pos;
        self.pos.line += 1;
    }

    /// Moves the cursor left by the specified distance, clamping at zero.
    #[inline]
    fn jump_left(&mut self, dist: usize) {
        self.previous_pos = self.pos;
        let dest = self.col() - dist;
        self.set_col(dest)
    }

    /// Moves the cursor right by the specified distance, clamping at the end of a row.
    #[inline]
    fn jump_right(&mut self, dist: usize, max: usize) {
        self.previous_pos = self.pos;
        let dest = usize::max(self.col() + dist, max);
        self.set_col(dest)
    }

    /// Moves the cursor up by the specified distance, clamping at the top.
    #[inline]
    fn jump_up(&mut self, dist: usize) {
        self.previous_pos = self.pos;
        let dest = self.line() - dist;
        self.set_line(dest);
    }

    /// Moves the cursor down by the specified distance, clamping at the bottom.
    #[inline]
    fn jump_down(&mut self, dist: usize) {
        self.previous_pos = self.pos;
        let dest = self.line() + dist;
        self.set_line(dest)
    }

    /// Updates the location the cursor points at depending on the current active modal state.
    fn mod_change(&mut self, modal: &Modal) {
        if self.plane.text() {
            if modal.is_visual_line() {
                self.last_text_mode_pos = LineCol {
                    line: self.pos.line,
                    col: 0,
                }
            } else {
                self.last_text_mode_pos = self.pos;
            }
            self.previous_pos = self.pos;
        }

        match modal {
            Modal::Command | Modal::Find(_) => {
                self.plane = CursorPlane::CommandBar;
                self.pos = LineCol { line: 0, col: 0 };
            }
            Modal::Normal | Modal::Insert | Modal::Visual | Modal::VisualLine => {
                self.plane = CursorPlane::Text;
                self.pos = self.last_text_mode_pos;
            }
        }
        self.pos_initial = LineCol {
            line: self.line(),
            col: self.col(),
        };
    }
}

/// Specifies at which plane the cursor is currently located.
#[derive(Clone, Debug)]
enum CursorPlane {
    Text,
    CommandBar,
    Terminal,
}
impl CursorPlane {
    const fn text(&self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match &self {
            Self::Text => true,
            _ => false,
        }
    }
}
