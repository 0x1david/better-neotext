use tracing::instrument;

use crate::{BaseAction, Component, LineCol, Modal};

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

#[derive(Debug)]
pub struct ShadowCursor {
    pub line: i64,
    pub col: i64,
}

impl From<&LineCol> for ShadowCursor {
    fn from(value: &LineCol) -> Self {
        Self {
            line: value.line as i64,
            col: value.col as i64,
        }
    }
}
impl ShadowCursor {
    pub fn update(&mut self, lc: LineCol) {
        self.line = lc.line as i64;
        self.col = lc.col as i64;
    }
}

impl Component for ShadowCursor {
    #[instrument]
    fn execute_action(&mut self, a: &BaseAction) -> crate::Result<()> {
        match a {
            BaseAction::MoveUp(dist) => self.line -= *dist as i64,
            BaseAction::MoveDown(dist) => self.line += *dist as i64,
            BaseAction::MoveLeft(dist) => self.col -= *dist as i64,
            BaseAction::MoveRight(dist) => self.col += *dist as i64,
            BaseAction::SetCursor(lc) => self.update(*lc),
            _ => (),
        };
        Ok(())
    }
}

impl Component for Cursor {
    #[instrument]
    fn execute_action(&mut self, a: &BaseAction) -> crate::Result<()> {
        match a {
            BaseAction::MoveUp(dist) => self.move_up(dist),
            BaseAction::MoveDown(dist) => self.move_down(dist),
            BaseAction::MoveLeft(dist) => self.move_left(dist),
            BaseAction::MoveRight(dist) => self.jump_right(dist),
            BaseAction::SetCursor(lc) => self.go(lc),
            BaseAction::ChangeMode(modal) => self.mod_change(modal),
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
        self.pos = *to;
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
    pub const fn text_mode_col(&self) -> usize {
        self.last_text_mode_pos.col
    }

    #[inline]
    pub fn set_col(&mut self, new: usize) {
        self.previous_pos = self.pos;
        self.pos.col = new;
    }

    /// Moves the cursor left by the specified distance, clamping at zero.
    #[inline]
    fn move_left(&mut self, dist: &usize) {
        self.previous_pos = self.pos;
        let dest = self.col() - dist;
        self.set_col(dest)
    }

    /// Moves the cursor right by the specified distance, clamping at the end of a row.
    #[inline]
    fn jump_right(&mut self, dist: &usize) {
        self.previous_pos = self.pos;
        let dest = self.col() + dist;
        self.set_col(dest)
    }

    /// Moves the cursor up by the specified distance, clamping at the top.
    #[inline]
    fn move_up(&mut self, dist: &usize) {
        self.previous_pos = self.pos;
        let dest = self.line() - dist;
        self.set_line(dest);
    }

    /// Moves the cursor down by the specified distance, clamping at the bottom.
    #[inline]
    fn move_down(&mut self, dist: &usize) {
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
