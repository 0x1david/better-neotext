use std::{borrow::Cow, cmp::Ordering, fmt::Display};
pub(crate) use crate::error::{Error, Result};

pub enum Action {
    Quit,
    Save,

    // Cursor Movement
    BumpUp,
    BumpDown,
    BumpLeft,
    BumpRight,
    JumpUp,
    JumpDown,
    SetCursor(LineCol),
    JumpLetter(char),
    ReverseJumpLetter(char),
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
    Find(Box<dyn Pattern>),
    ReverseFind(Box<dyn Pattern>),
    FindChar(char),
    ReverseFindChar(char),

    // Insertions

    // Text Manipulation
    Replace(char),
    InsertCharAtCursor(char),
    InsertNewline,
    InsertBelow,
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
    GetUnderCursor,
    OpenFile,

    Nothing
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
#[derive(Default, Debug, PartialEq, Eq)]
pub enum Modal {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    Find(FindMode),
    Command,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum FindMode {
    #[default]
    Forwards,
    Backwards,
}

pub enum Command {
    Find,
    Rfind,
    Leave,
}
