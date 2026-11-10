use std::{
  io::{self, Stdout, Write},
  time::{SystemTime, UNIX_EPOCH},
};

use crossterm::{
  cursor::MoveTo,
  queue,
  style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
  terminal::{self, Clear, ClearType},
};
use time::{OffsetDateTime, UtcOffset};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::scan::DirectoryEntry;

use super::{App, ScanStatus, TimestampState};

const MARKER_WIDTH: usize = 2;
pub(super) const TIMESTAMP_WIDTH: usize = 16;
const TIMESTAMP_GAP: usize = 4;
const MIN_NAME_WIDTH: usize = 4;
pub(super) const MAX_ENTRY_LINE_WIDTH: usize = 80;
const TIMESTAMP_FORMAT: &[time::format_description::FormatItem<'static>] =
  time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]");

impl App {
  pub(super) fn draw(&self, output: &mut Stdout) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    queue!(output, MoveTo(0, 0), Clear(ClearType::All))?;
    put_line(
      output,
      0,
      &format!(" FAST  {}", self.current_dir.display()),
      width,
      Color::Cyan,
      false,
    )?;
    put_line(
      output,
      1,
      &self.status_text(),
      width,
      Color::DarkGrey,
      false,
    )?;

    let list_height = height.saturating_sub(3) as usize;
    let files_position = self.files_position();
    let name_column_width = self.name_column_width(width);
    let selected_row = self.selected_row(files_position);
    let scroll_start = self.scroll_start(list_height, selected_row);
    for row in 0..list_height {
      let row_index = scroll_start + row;
      if files_position == Some(row_index) {
        put_line(
          output,
          row as u16 + 2,
          " -- Files --",
          width,
          Color::DarkGrey,
          false,
        )?;
        continue;
      }
      let index = row_index.saturating_sub(usize::from(
        files_position.is_some_and(|position| row_index > position),
      ));
      let Some(&entry_index) = self.visible_indices.get(index) else {
        put_line(output, row as u16 + 2, "", width, Color::Reset, false)?;
        continue;
      };
      let Some(entry) = self.entries.get(entry_index) else {
        put_line(output, row as u16 + 2, "", width, Color::Reset, false)?;
        continue;
      };
      let marker = if index == self.selected { "> " } else { "  " };
      let timestamp = self.timestamp_text(entry);
      let text = format_entry_line(marker, &entry.name, &timestamp, width, name_column_width);
      put_line(
        output,
        row as u16 + 2,
        &text,
        width,
        Self::entry_color(entry),
        index == self.selected,
      )?;
    }

    put_line(
      output,
      height.saturating_sub(1),
      &self.footer_text(),
      width,
      Color::DarkGrey,
      false,
    )?;
    output.flush()
  }

  pub(super) fn files_position(&self) -> Option<usize> {
    if !self.show_files {
      return None;
    }
    // Sorting and filtering keep directories before non-directory entries.
    let position = self
      .visible_indices
      .partition_point(|&index| self.entries[index].is_directory);
    self
      .visible_indices
      .get(position)
      .is_some_and(|&index| !self.entries[index].is_directory)
      .then_some(position)
  }

  pub(super) fn selected_row(&self, files_position: Option<usize>) -> usize {
    self.selected + usize::from(files_position.is_some_and(|position| position <= self.selected))
  }

  pub(super) fn entry_color(entry: &DirectoryEntry) -> Color {
    if entry.is_directory {
      Color::White
    } else {
      Color::DarkGrey
    }
  }

  fn status_text(&self) -> String {
    let status = match &self.status {
      ScanStatus::Indexing => {
        format!(
          " Indexing... {} {} discovered",
          self.discovered_count(),
          self.entry_label()
        )
      }
      ScanStatus::Refreshing => {
        format!(
          " Refreshing timestamps... {} {}",
          self.discovered_count(),
          self.entry_label()
        )
      }
      ScanStatus::Ready => format!(" Ready  {} {}", self.discovered_count(), self.entry_label()),
      ScanStatus::Error(error) => format!(" Error  {error}"),
    };
    if self.filter_mode {
      format!(
        "{status}  Filter ({}): {}_",
        self.filter_kind.label(),
        self.filter_query
      )
    } else if self.filter_query.is_empty() {
      status
    } else {
      format!(
        "{status}  Filter ({}): {} ({} visible)",
        self.filter_kind.label(),
        self.filter_query,
        self.visible_indices.len()
      )
    }
  }

  fn footer_text(&self) -> String {
    if self.filter_mode {
      " Type to filter  Tab toggle mode  Backspace edit  Enter keep  Esc clear  Ctrl-C cancel"
        .to_owned()
    } else if self.filter_query.is_empty() {
      format!(
        " / filter  Up/Down or j/k  Enter/l open  F files {}  y copy path  Backspace/h parent  r rescan  q select  Esc cancel",
        if self.show_files { "on" } else { "off" }
      )
    } else {
      format!(
        " / edit filter  Tab toggle mode  Up/Down or j/k  Enter/l open  F files {}  y copy path  Backspace/h parent  r rescan  q select  Esc clear",
        if self.show_files { "on" } else { "off" }
      )
    }
  }

  fn entry_label(&self) -> &'static str {
    if self.show_files {
      "entries"
    } else {
      "directories"
    }
  }

  pub(super) fn scroll_start(&self, list_height: usize, selected_row: usize) -> usize {
    if list_height == 0 {
      return 0;
    }
    selected_row.saturating_sub(list_height.saturating_sub(1))
  }

  pub(super) fn name_column_width(&self, width: usize) -> Option<usize> {
    let available = width
      .min(MAX_ENTRY_LINE_WIDTH)
      .saturating_sub(MARKER_WIDTH + TIMESTAMP_GAP + TIMESTAMP_WIDTH);
    (available >= MIN_NAME_WIDTH).then_some(
      self
        .max_visible_name_width
        .max(MIN_NAME_WIDTH)
        .min(available),
    )
  }

  fn timestamp_text(&self, entry: &DirectoryEntry) -> String {
    match self
      .timestamps
      .get(&entry.path)
      .cloned()
      .unwrap_or(match &self.status {
        ScanStatus::Indexing | ScanStatus::Refreshing => TimestampState::Pending,
        ScanStatus::Ready | ScanStatus::Error(_) => TimestampState::Unavailable,
      }) {
      TimestampState::Pending => "...".to_owned(),
      TimestampState::Available(modified) => {
        format_timestamp(modified).unwrap_or_else(|| "-".to_owned())
      }
      TimestampState::Unavailable => "-".to_owned(),
    }
  }
}

pub(super) fn format_timestamp(modified: SystemTime) -> Option<String> {
  let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
  let timestamp = OffsetDateTime::from_unix_timestamp_nanos(elapsed.as_nanos() as i128).ok()?;
  let offset = UtcOffset::local_offset_at(timestamp).ok()?;
  let formatted = timestamp.to_offset(offset).format(TIMESTAMP_FORMAT).ok()?;
  (UnicodeWidthStr::width(formatted.as_str()) == TIMESTAMP_WIDTH).then_some(formatted)
}

pub(super) fn format_entry_line(
  marker: &str,
  name: &str,
  timestamp: &str,
  width: usize,
  name_column_width: Option<usize>,
) -> String {
  let marker_width = UnicodeWidthStr::width(marker);
  let Some(name_column_width) = name_column_width else {
    return format!(
      "{marker}{}",
      truncate_with_ellipsis(name, width.saturating_sub(marker_width))
    );
  };
  if width < marker_width + name_column_width + TIMESTAMP_GAP + TIMESTAMP_WIDTH {
    return format!(
      "{marker}{}",
      truncate_with_ellipsis(name, width.saturating_sub(marker_width))
    );
  }

  let name = truncate_with_ellipsis(name, name_column_width);
  let name_padding = name_column_width.saturating_sub(UnicodeWidthStr::width(name.as_str()));
  let name_padding = " ".repeat(name_padding);
  let gap = " ".repeat(TIMESTAMP_GAP);
  format!("{marker}{name}{name_padding}{gap}{timestamp}")
}

fn truncate_with_ellipsis(text: &str, width: usize) -> String {
  if UnicodeWidthStr::width(text) <= width {
    return text.to_owned();
  }
  if width <= 3 {
    return take_display_width(text, width);
  }
  format!("{}...", take_display_width(text, width - 3))
}

fn take_display_width(text: &str, width: usize) -> String {
  let mut result = String::new();
  let mut used = 0;
  for character in text.chars() {
    let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
    if used + character_width > width {
      break;
    }
    result.push(character);
    used += character_width;
  }
  result
}

fn put_line<W: Write>(
  output: &mut W,
  row: u16,
  text: &str,
  width: usize,
  color: Color,
  selected: bool,
) -> io::Result<()> {
  let text = take_display_width(text, width);
  queue!(
    output,
    MoveTo(0, row),
    Clear(ClearType::UntilNewLine),
    SetForegroundColor(color)
  )?;
  if selected {
    queue!(output, SetAttribute(Attribute::Reverse))?;
  }
  queue!(
    output,
    Print(text),
    SetAttribute(Attribute::Reset),
    ResetColor
  )
}
