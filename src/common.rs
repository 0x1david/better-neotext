pub(crate) use crate::error::{Error, Result};
use crate::{cursor::Cursor, editor::Lazy};
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Display},
};

pub trait Component {
    fn execute_action(&mut self, a: &BaseAction) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum BaseAction {
    Save,

    MoveUp(usize),
    MoveDown(usize),
    MoveRight(usize),
    MoveLeft(usize),
    SetCursor(LineCol),

    ChangeMode(Modal),

    Yank,
    Paste(char, usize),

    InsertAt(Lazy<LineCol>, char),
    DeleteAt(Lazy<LineCol>, usize),
    DeleteLineAt(Lazy<LineCol>, usize),

    ExecuteCommand(Command),

    Undo(usize),
    Redo(usize),
    FetchFromHistory,

    GetUnderCursor,
    OpenFile,

    Nothing,
}
impl BaseAction {
    /// Multiply the repeat factor of an action by x
    pub fn repeat(mut self, x: usize) -> Self {
        if let Some(n) = self.get_repeater() {
            *n *= x;
        };
        self
    }
    /// Get the number of times an action is being repeated (if repeatable)
    fn get_repeater(&mut self) -> Option<&mut usize> {
        match self {
            Self::MoveUp(n)
            | Self::MoveDown(n)
            | Self::MoveLeft(n)
            | Self::MoveRight(n)
            | Self::Undo(n)
            | Self::Redo(n)
            | Self::DeleteAt(_, n)
            | Self::DeleteLineAt(_, n) => Some(n),
            Self::Paste(_, n) => Some(n),
            _ => None,
        }
    }
}

pub trait Pattern {
    /// The caller has two main responsibilities:
    ///     1. Preprocessing the haystack in such a way that only the part to be searched is
    ///        provided
    ///     2. Adding the column number that we were starting from if the match is found on the
    ///        first line of the search (if returned linecol.line equals the cursor position)
    ///
    /// Thus find and rfind will require to be split at the cursor
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol>;
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol>;
}

impl Pattern for &str {
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .find_map(|(line_num, line_content)| {
                line_content.find(self).map(|col| LineCol {
                    line: line_num,
                    col,
                })
            })
    }
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .rev()
            .find_map(|(line_num, line_content)| {
                line_content.rfind(self).map(|col| LineCol {
                    line: line_num,
                    col,
                })
            })
    }
}

// impl<F> Pattern for F
// where
//     F: Fn(&str) -> Option<usize>,
// {
//     fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
//         haystack
//             .iter()
//             .enumerate()
//             .find_map(|(line_num, line_content)| {
//                 self(line_content.as_ref()).map(|col| LineCol {
//                     line: line_num,
//                     col,
//                 })
//             })
//     }
// }

impl Pattern for String {
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        self.as_str().find_pattern(haystack)
    }
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        self.as_str().rfind_pattern(haystack)
    }
}

impl Pattern for Cow<'_, str> {
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        self.as_ref().find_pattern(haystack)
    }
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        self.as_ref().rfind_pattern(haystack)
    }
}

impl Pattern for char {
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .find_map(|(line_num, line_content)| {
                line_content.find(*self).map(|col| LineCol {
                    line: line_num,
                    col,
                })
            })
    }
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .rev()
            .find_map(|(line_num, line_content)| {
                line_content.rfind(*self).map(|col| LineCol {
                    line: line_num,
                    col,
                })
            })
    }
}

impl<F> Pattern for F
where
    F: Fn(char) -> bool,
{
    fn find_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .find_map(|(line_num, line_content)| {
                line_content.chars().position(self).map(|col| LineCol {
                    line: line_num,
                    col,
                })
            })
    }
    fn rfind_pattern(&self, haystack: &[String]) -> Option<LineCol> {
        haystack
            .iter()
            .enumerate()
            .rev()
            .find_map(|(line_num, line_content)| {
                line_content
                    .chars()
                    .rev()
                    .position(self)
                    .map(|rcol| LineCol {
                        line: line_num,
                        col: line_content.len() - rcol,
                    })
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

impl Display for LineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl PartialOrd for LineCol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.line.cmp(&other.line) {
            Ordering::Equal => self.col.cmp(&other.col).into(),
            otherwise => Some(otherwise),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub start: LineCol,
    pub end: LineCol,
}

impl From<&Cursor> for Selection {
    fn from(value: &Cursor) -> Self {
        Self {
            start: value.last_text_mode_pos,
            end: value.pos,
        }
    }
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

/// Contains the main modal variants of the editor.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Modal {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    Find(FindMode),
    Command,
}

impl Display for Modal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match self {
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "VISUAL_LINE",
            Self::Command => "COMMAND",
            Self::Normal => "NORMAL",
            Self::Find(FindMode::Forwards) => "FW_FIND",
            Self::Find(FindMode::Backwards) => "BW_FIND",
        };
        write!(f, "{}", repr)
    }
}

impl Modal {
    pub fn is_normal(&self) -> bool {
        matches!(&self, Modal::Normal)
    }
    pub fn is_insert(&self) -> bool {
        matches!(&self, Modal::Insert)
    }
    pub fn is_visual(&self) -> bool {
        matches!(&self, Modal::Visual)
    }
    pub fn is_visual_line(&self) -> bool {
        matches!(&self, Modal::VisualLine)
    }
    pub fn is_command(&self) -> bool {
        matches!(&self, Modal::Command)
    }
    pub fn is_any_find(&self) -> bool {
        matches!(&self, Modal::Find(_))
    }
    pub fn is_forwards_find(&self) -> bool {
        matches!(&self, Modal::Find(FindMode::Forwards))
    }
    pub fn is_backwards_find(&self) -> bool {
        matches!(&self, Modal::Find(FindMode::Backwards))
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum FindMode {
    #[default]
    Forwards,
    Backwards,
}

#[derive(Clone, Debug)]
pub enum Command {
    Find,
    Rfind,
    Leave,
}
