use crate::{cursor::Cursor, BaseAction, Component, Modal, Result, Selection};
use std::io::{self, Stdout, Write};

use crossterm::{
    execute,
    style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, LeaveAlternateScreen},
};

const NO_OF_BARS: u8 = 2;
pub const LINE_NUMBER_SEPARATOR_EMPTY_COLUMNS: usize = 4;
pub const LINE_NUMBER_RESERVED_COLUMNS: usize = 5;

#[derive(Debug)]
pub struct ViewPort {
    terminal: Stdout,
    width: u16,
    height: u16,
    top_border: usize,
    bottom_border: usize,
    mode: Modal,
}

impl Component for ViewPort {
    fn execute_action(&mut self, a: &BaseAction) -> Result<()> {
        println!("Executing Action at Viewport: {:?}", a);
        match a {
            BaseAction::MoveUp(dist) => self.move_up(*dist),
            BaseAction::MoveDown(dist) => self.move_down(*dist),
            _ => (),
        };
        Ok(())
    }
}

impl ViewPort {
    fn move_up(&mut self, dist: usize) {
        self.top_border -= dist;
        self.bottom_border -= dist;
    }
    fn move_down(&mut self, dist: usize) {
        self.top_border += dist;
        self.bottom_border += dist;
    }
}

impl Drop for ViewPort {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            self.terminal,
            terminal::Clear(ClearType::All),
            LeaveAlternateScreen
        );
    }
}

impl Default for ViewPort {
    fn default() -> Self {
        terminal::enable_raw_mode().expect("Couldn't start up terminal in raw mode.");
        let terminal = io::stdout();
        let (width, height) = terminal::size().expect("Failed reading terminal information");
        Self {
            terminal,
            width,
            height,
            top_border: 0,
            bottom_border: height as usize,
            mode: Modal::Normal,
        }
    }
}

impl ViewPort {
    pub fn update_viewport(&mut self, buf: &[String], cursor: &Cursor) -> Result<()> {
        // Prepare Viewport
        (self.width, self.height) = terminal::size().expect("Failed reading terminal information");
        execute!(
            self.terminal,
            terminal::Clear(ClearType::All),
            crossterm::cursor::MoveTo(0, 0),
        )?;

        // Calculate the range of lines to display
        let start = self.top_border;
        let end = self.bottom_border.saturating_sub(NO_OF_BARS as usize);
        let visible_lines = end.saturating_sub(start) + 1;

        // Create an iterator that pads with empty strings if out of bounds
        let padded_iter = buf[start..]
            .iter()
            .map(|s| s.as_str())
            .chain(std::iter::repeat(""))
            .take(visible_lines);

        // Write Content
        for (i, line) in padded_iter.enumerate() {
            let line_number = start + i;
            execute!(self.terminal, terminal::Clear(ClearType::CurrentLine))?;
            self.create_line_numbers(line_number + 1, cursor.line())?;
            self.draw_line(line, line_number, cursor)?;
        }

        Ok(())
    }

    fn create_line_numbers(&mut self, line_number: usize, cursor_line: usize) -> Result<()> {
        execute!(self.terminal, SetForegroundColor(Color::Green))?;
        let rel_line_number = (line_number as i64 - cursor_line as i64 - 1).abs();
        let line_number = if rel_line_number == 0 {
            line_number as i64
        } else {
            rel_line_number
        };

        print!(
            "{line_number:>width$}{separator}",
            line_number = line_number,
            width = LINE_NUMBER_RESERVED_COLUMNS,
            separator = " ".repeat(LINE_NUMBER_SEPARATOR_EMPTY_COLUMNS)
        );
        execute!(self.terminal, ResetColor)?;
        Ok(())
    }

    fn draw_line(
        &mut self,
        line: impl AsRef<str>,
        absolute_ln: usize,
        cursor: &Cursor,
    ) -> Result<()> {
        let line = line.as_ref();
        let selection = Selection::from(cursor).normalized();

        let line_in_highlight_bounds =
            absolute_ln >= selection.start.line && absolute_ln <= selection.end.line;
        let highlight_whole_line = (self.mode.is_visual_line() && line_in_highlight_bounds)
            || absolute_ln > selection.start.line
                && (absolute_ln < selection.end.line.saturating_sub(1) && self.mode.is_visual());

        // Decide on which parts to highlight
        if highlight_whole_line {
            execute!(
                self.terminal,
                SetBackgroundColor(Color::White),
                SetForegroundColor(Color::Black)
            )?;
            write!(self.terminal, "{}\r", line)?;
            execute!(self.terminal, ResetColor)?;
        } else if self.mode.is_visual() && line_in_highlight_bounds {
            let start_col = if absolute_ln == selection.start.line {
                selection.start.col
            } else {
                0
            };
            let end_col = if absolute_ln == selection.end.line {
                selection.end.col
            } else {
                line.len()
            };

            // Write line - before Selection
            write!(self.terminal, "{}", &line[..start_col])?;

            // Write Whole Selection
            execute!(
                self.terminal,
                SetBackgroundColor(Color::White),
                SetForegroundColor(Color::Black)
            )?;
            write!(self.terminal, "{}", &line[start_col..end_col])?;
            execute!(self.terminal, ResetColor)?;

            // Print last line - after selection
            write!(self.terminal, "{}\r", &line[end_col..])?;
        } else {
            write!(self.terminal, "{}\r", line)?;
        }

        writeln!(self.terminal)?;
        Ok(())
    }
}
