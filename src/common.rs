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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseAction {
    Save,

    MoveUp(usize),
    MoveDown(usize),
    MoveRight(usize),
    MoveLeft(usize),
    // `SetCursor` should only ever be used when bound checking is not required
    // It is NOT bound checked. For bound checked movement use `Move` commands
    SetCursor(LineCol),

    ChangeMode(Modal),

    Yank,
    Paste(char, usize),

    InsertAt(Lazy<LineCol>, char),
    InsertLineAt(Lazy<LineCol>, usize),
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

#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub enum FindDirection {
    Forwards,
    Backwards,
}

/// Contains the main modal variants of the editor.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Modal {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    Find(FindDirection),
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
            Self::Find(FindDirection::Forwards) => "FORWARD FIND",
            Self::Find(FindDirection::Backwards) => "BACKWARD FIND",
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
    pub fn is_find(&self) -> bool {
        matches!(&self, Modal::Find(_))
    }
    pub fn is_forwards_find(&self) -> bool {
        matches!(&self, Modal::Find(FindDirection::Forwards))
    }
    pub fn is_backwards_find(&self) -> bool {
        matches!(&self, Modal::Find(FindDirection::Backwards))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Find(String),
    Rfind(String),
    Exit,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer() -> Vec<String> {
        vec![
            "Hello, world!".to_string(),
            "This is a test.".to_string(),
            "12345".to_string(),
            "   Spaces   ".to_string(),
        ]
    }

    #[test]
    fn test_str_pattern() {
        let buffer = create_test_buffer();
        let pattern = "world";
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 7 })
        );
    }

    #[test]
    fn test_string_pattern() {
        let buffer = create_test_buffer();
        let pattern = "test".to_string();
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 1, col: 10 })
        );
    }

    #[test]
    fn test_cow_str_pattern() {
        let buffer = create_test_buffer();
        let pattern: Cow<str> = Cow::Borrowed("This");
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 1, col: 0 })
        );
    }

    #[test]
    fn test_char_pattern() {
        let buffer = create_test_buffer();
        let pattern = '5';
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 2, col: 4 })
        );
    }

    #[test]
    fn test_char_predicate_pattern() {
        let buffer = create_test_buffer();
        let pattern = |c: char| c.is_ascii_digit();
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 2, col: 0 })
        );
    }

    #[test]
    fn test_pattern_not_found() {
        let buffer = create_test_buffer();
        let pattern = "nonexistent";
        assert_eq!(pattern.find_pattern(&buffer), None);
    }
    #[test]
    fn test_empty_buffer() {
        let buffer: Vec<String> = Vec::new();
        let pattern = "test";
        assert_eq!(pattern.find_pattern(&buffer), None);
    }

    #[test]
    fn test_pattern_at_start() {
        let buffer = vec!["Start here".to_string()];
        let pattern = "Start";
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 0 })
        );
    }

    #[test]
    fn test_pattern_at_end() {
        let buffer = vec!["End here".to_string()];
        let pattern = "here";
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 4 })
        );
    }

    #[test]
    fn test_pattern_spanning_lines() {
        let buffer = vec!["First ".to_string(), "line".to_string()];
        let pattern = "First line";
        assert_eq!(pattern.find_pattern(&buffer), None);
    }

    #[test]
    fn test_char_pattern_whitespace() {
        let buffer = vec!["No space".to_string(), " Leading space".to_string()];
        let pattern = ' ';
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 2 })
        );
    }

    #[test]
    fn test_char_predicate_uppercase() {
        let buffer = vec!["lowercase".to_string(), "UPPERCASE".to_string()];
        let pattern = |c: char| c.is_ascii_uppercase();
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 1, col: 0 })
        );
    }

    #[test]
    fn test_pattern_with_special_chars() {
        let buffer = vec!["Special: !@#$%^&*()".to_string()];
        let pattern = "$%^&";
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 12 })
        );
    }

    #[test]
    fn test_pattern_case_sensitivity() {
        let buffer = vec!["Case Sensitive".to_string()];
        let pattern = "case";
        assert_eq!(pattern.find_pattern(&buffer), None);
    }

    #[test]
    fn test_pattern_overlapping() {
        let buffer = vec!["aaa".to_string()];
        let pattern = "aa";
        assert_eq!(
            pattern.find_pattern(&buffer),
            Some(LineCol { line: 0, col: 0 })
        );
    }
    #[test]
    fn test_sequential_char_predicates() {
        let buffer = vec![
            "First line with some numbers 123".to_string(),
            "Second line without numbers".to_string(),
            "Third line with 456 and more text".to_string(),
            "Fourth line ends with numbers 789".to_string(),
        ];

        let predicate1 = |c: char| c.is_ascii_digit();

        let predicate2 = |c: char| c.is_ascii_uppercase();

        let result1 = predicate1.find_pattern(&buffer);
        assert_eq!(result1, Some(LineCol { line: 0, col: 29 }));

        let result2 = predicate2.find_pattern(&buffer[result1.unwrap().line + 1..]);
        assert_eq!(result2, Some(LineCol { line: 0, col: 0 })); // 'S' in "Second"

        let final_result = result2.map(|lc| LineCol {
            line: lc.line + result1.unwrap().line + 1,
            col: lc.col,
        });
        assert_eq!(final_result, Some(LineCol { line: 1, col: 0 }));
    }
}
