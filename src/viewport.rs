use std::io::{self, Stdout, Write};
use crate::{cursor::Cursor, Component, Result, Selection};

use crossterm::{execute, style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor}, terminal::{self, ClearType, LeaveAlternateScreen}};

const NO_OF_BARS: u8 = 2;


pub struct ViewPort {
    terminal: Stdout,
    width: u16,
    height: u16,
    top_border: usize,
    bottom_border: usize,
}

impl Component for ViewPort {
    fn execute_action(&mut self, a: &crate::Action) -> Result<()> {
        match a {
            _ => todo!()
        }
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
            height
        }
    }
}

impl ViewPort {
    pub fn update_viewport(&mut self, buf: &[String], cursor_line: usize) -> Result<()> {

        // Prepare Viewport
        (self.width, self.height) = terminal::size().expect("Failed reading terminal information");
        execute!(
            self.terminal,
            terminal::Clear(ClearType::All),
            crossterm::cursor::MoveTo(0, 0),
            )?;

        // Write Content
        for (i, line) in buf[self.top_border..=self.bottom_border.saturating_sub(NO_OF_BARS as usize)]
            .iter()
            .enumerate()
        {
            let line_number = self.top_border + i;

            execute!(self.terminal, terminal::Clear(ClearType::CurrentLine))?;

            self.create_line_numbers(&mut stdout, line_number + 1)?;
            self.draw_line(line, line_number)?;
        }

        Ok(())
    }

    fn draw_line(&self, line: impl AsRef<str>, absolute_ln: usize, cursor: &Cursor) -> Result<()> {
        let line = line.as_ref();
        let selection = Selection::from(cursor).normalized();

        let line_in_highlight_bounds =
            absolute_ln >= selection.start.line && absolute_ln <= selection.end.line;
        let highlight_whole_line = (self.mode.is_visual_line() && line_in_highlight_bounds)
            || absolute_ln > selection.start.line
                && (absolute_ln < selection.end.line.saturating_sub(1) && self.mode.is_visual());

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

            write!(self.terminal, "{}", &line[..start_col])?;

            execute!(
                self.terminal,
                SetBackgroundColor(Color::White),
                SetForegroundColor(Color::Black)
            )?;
            write!(self.terminal, "{}", &line[start_col..end_col])?;
            execute!(self.terminal, ResetColor)?;

            // Print part after selection
            write!(self.terminal, "{}\r", &line[end_col..])?;
        } else {
            write!(self.terminal, "{}\r", line)?;
        }

        writeln!(self.terminal)?;
        Ok(())
    }
}
