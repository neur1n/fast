use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
  ExitAction, KeyAction, TimestampState, navigation_entries, tests::app_with_entries,
};

use super::*;

fn app() -> App {
  let current_dir = PathBuf::from("/tmp/current");
  let mut entries = navigation_entries(&current_dir);
  entries.extend((0..8).map(|index| DirectoryEntry {
    name: format!("dir{index}"),
    path: current_dir.join(format!("dir{index}")),
    is_directory: true,
  }));
  app_with_entries(current_dir, entries, 1)
}

fn render(renderer: &mut Renderer, frame: Frame) -> Vec<(u16, Line)> {
  let mut lines = Vec::new();
  renderer
    .draw_frame(&mut Vec::new(), frame, |output, row, line| {
      lines.push((row, line.clone()));
      output.write_all(line.text.as_bytes())
    })
    .unwrap();
  lines
}

fn row_numbers(lines: &[(u16, Line)]) -> Vec<u16> {
  lines.iter().map(|(row, _)| *row).collect()
}

struct UnexpectedOutput;

impl Write for UnexpectedOutput {
  fn write(&mut self, _: &[u8]) -> io::Result<usize> {
    panic!("an unchanged frame must not write");
  }

  fn flush(&mut self) -> io::Result<()> {
    panic!("an unchanged frame must not flush");
  }
}

#[test]
fn first_frame_updates_every_visible_row() {
  let mut renderer = Renderer::default();
  let lines = render(&mut renderer, app().frame((120, 10)));

  assert_eq!(row_numbers(&lines), (0..10).collect::<Vec<_>>());
  assert!(lines[0].1.text.starts_with(" FAST  "));
  assert!(lines[9].1.text.starts_with(" / filter"));
}

#[test]
fn identical_frame_emits_no_commands_or_flush() {
  let mut renderer = Renderer::default();
  let frame = app().frame((120, 10));
  render(&mut renderer, frame.clone());

  renderer
    .draw_frame(&mut UnexpectedOutput, frame, |_, _, _| {
      panic!("an unchanged frame must not redraw a line");
    })
    .unwrap();
}

#[test]
fn selection_movement_only_updates_old_and_new_rows() {
  let mut renderer = Renderer::default();
  let mut app = app();
  render(&mut renderer, app.frame((120, 10)));

  app.selected = 2;
  let lines = render(&mut renderer, app.frame((120, 10)));

  assert_eq!(row_numbers(&lines), vec![3, 4]);
  assert!(!lines[0].1.selected);
  assert!(lines[0].1.text.starts_with("  "));
  assert!(lines[1].1.selected);
  assert!(lines[1].1.text.starts_with("> "));
}

#[test]
fn scrolling_only_updates_the_list_region() {
  let mut renderer = Renderer::default();
  let mut app = app();
  app.selected = 4;
  render(&mut renderer, app.frame((120, 8)));

  app.selected = 5;
  let lines = render(&mut renderer, app.frame((120, 8)));

  assert_eq!(row_numbers(&lines), (2..7).collect::<Vec<_>>());
  assert!(lines.last().unwrap().1.selected);
}

#[test]
fn style_changes_are_not_skipped() {
  let mut renderer = Renderer::default();
  let mut frame = Frame::new((20, 4));
  frame.set_line(2, "same text", Color::White, false);
  render(&mut renderer, frame.clone());

  frame.lines[2].color = Color::DarkGrey;
  let lines = render(&mut renderer, frame.clone());
  assert_eq!(row_numbers(&lines), vec![2]);
  assert_eq!(lines[0].1.color, Color::DarkGrey);

  frame.lines[2].selected = true;
  let lines = render(&mut renderer, frame);
  assert_eq!(row_numbers(&lines), vec![2]);
  assert_eq!(lines[0].1.text, "same text");
  assert!(lines[0].1.selected);
}

#[test]
fn filtering_clears_rows_from_the_previous_listing() {
  let mut renderer = Renderer::default();
  let mut app = app();
  render(&mut renderer, app.frame((120, 10)));

  app.set_filter_query("does-not-match".to_owned());
  let lines = render(&mut renderer, app.frame((120, 10)));

  assert_eq!(app.visible_indices.len(), 2);
  for row in 4..9 {
    let (_, line) = lines.iter().find(|(index, _)| *index == row).unwrap();
    assert!(line.text.is_empty());
    assert_eq!(line.color, Color::Reset);
    assert!(!line.selected);
  }
  assert!(
    lines
      .iter()
      .any(|(row, line)| *row == 9 && line.text.starts_with(" / edit filter"))
  );
}

#[test]
fn filter_mode_updates_and_restores_the_footer() {
  let mut renderer = Renderer::default();
  let mut app = app();
  render(&mut renderer, app.frame((120, 10)));

  app.filter_mode = true;
  let lines = render(&mut renderer, app.frame((120, 10)));
  assert!(
    lines
      .iter()
      .any(|(row, line)| *row == 9 && line.text == " Tab toggle mode  Enter apply  Esc clear")
  );

  app.filter_mode = false;
  let lines = render(&mut renderer, app.frame((120, 10)));
  assert!(
    lines
      .iter()
      .any(|(row, line)| *row == 9 && line.text.starts_with(" / filter"))
  );
}

#[test]
fn browse_footer_only_lists_core_actions_and_file_state() {
  let mut app = app();
  for (show_files, state) in [(false, "off"), (true, "on")] {
    app.show_files = show_files;
    app.set_filter_query(String::new());
    assert_eq!(
      app.footer_text(),
      format!(" / filter  q select  r rescan  y copy path  F files {state}  Esc cancel")
    );

    app.set_filter_query("dir".to_owned());
    assert_eq!(
      app.footer_text(),
      format!(" / edit filter  q select  r rescan  y copy path  F files {state}  Esc clear/cancel")
    );
    assert!(UnicodeWidthStr::width(app.footer_text().as_str()) <= 79);
  }
}

#[test]
fn filter_footer_lists_core_actions_and_preserves_escape_behavior() {
  let mut app = app();
  app.set_filter_query("dir".to_owned());
  app.filter_mode = true;
  assert_eq!(
    app.footer_text(),
    " Tab toggle mode  Enter apply  Esc clear"
  );
  assert!(UnicodeWidthStr::width(app.footer_text().as_str()) <= 79);

  let escape = || KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
  assert!(matches!(app.handle_key(escape()), Some(KeyAction::Redraw)));
  assert!(!app.filter_mode);
  assert!(app.filter_query.is_empty());
  assert!(app.footer_text().ends_with("Esc cancel"));
  assert!(matches!(
    app.handle_key(escape()),
    Some(KeyAction::Exit(ExitAction::Cancel))
  ));
}

#[test]
fn kept_filter_footer_matches_escape_clear_then_cancel() {
  let mut app = app();
  app.set_filter_query("dir".to_owned());
  assert!(app.footer_text().ends_with("Esc clear/cancel"));

  let escape = || KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
  assert!(matches!(app.handle_key(escape()), Some(KeyAction::Redraw)));
  assert!(app.filter_query.is_empty());
  assert!(app.footer_text().ends_with("Esc cancel"));
  assert!(matches!(
    app.handle_key(escape()),
    Some(KeyAction::Exit(ExitAction::Cancel))
  ));
}

#[test]
fn timestamp_changes_only_update_the_affected_entry() {
  let mut renderer = Renderer::default();
  let mut app = app();
  let path = app.entries[2].path.clone();
  app.timestamps.insert(path.clone(), TimestampState::Pending);
  render(&mut renderer, app.frame((120, 10)));

  app.timestamps.insert(path, TimestampState::Unavailable);
  let lines = render(&mut renderer, app.frame((120, 10)));

  assert_eq!(row_numbers(&lines), vec![4]);
  assert!(lines[0].1.text.ends_with('-'));
}

#[test]
fn file_header_and_removed_file_rows_are_updated() {
  let mut renderer = Renderer::default();
  let mut app = app();
  app.show_files = true;
  render(&mut renderer, app.frame((120, 16)));

  app.apply_entries(vec![DirectoryEntry {
    name: "file".to_owned(),
    path: app.current_dir.join("file"),
    is_directory: false,
  }]);
  let lines = render(&mut renderer, app.frame((120, 16)));
  assert!(
    lines
      .iter()
      .any(|(row, line)| *row == 12 && line.text == " -- Files --")
  );
  assert!(
    lines
      .iter()
      .any(|(row, line)| *row == 13 && line.color == Color::DarkGrey)
  );
  assert!(!row_numbers(&lines).contains(&15));

  app.entries.retain(|entry| entry.is_directory);
  app.refresh_visible();
  let lines = render(&mut renderer, app.frame((120, 16)));
  for row in [12, 13] {
    let (_, line) = lines.iter().find(|(index, _)| *index == row).unwrap();
    assert!(line.text.is_empty());
    assert_eq!(line.color, Color::Reset);
  }
  assert!(!row_numbers(&lines).contains(&15));
}

#[test]
fn resizing_redraws_all_visible_rows() {
  let mut renderer = Renderer::default();
  let app = app();
  render(&mut renderer, app.frame((120, 10)));

  for size in [(80, 10), (80, 13), (80, 5)] {
    let lines = render(&mut renderer, app.frame(size));
    assert_eq!(row_numbers(&lines), (0..size.1).collect::<Vec<_>>());
    assert!(lines.last().unwrap().1.text.starts_with(" / filter"));
  }
}

#[test]
fn clipped_text_changes_do_not_redraw_the_same_visible_line() {
  let mut renderer = Renderer::default();
  let mut frame = Frame::new((5, 3));
  frame.set_line(0, "abcde-first", Color::White, false);
  render(&mut renderer, frame.clone());

  frame.set_line(0, "abcde-second", Color::White, false);
  assert!(render(&mut renderer, frame).is_empty());
}

#[test]
fn bottom_line_leaves_the_last_column_unused() {
  let mut frame = app().frame((10, 5));

  assert_eq!(UnicodeWidthStr::width(frame.lines[0].text.as_str()), 10);
  assert_eq!(UnicodeWidthStr::width(frame.lines[4].text.as_str()), 9);

  frame.set_line(4, "目录目录目录", Color::White, false);
  assert_eq!(UnicodeWidthStr::width(frame.lines[4].text.as_str()), 8);
}

#[test]
fn zero_and_tiny_sizes_only_address_existing_rows() {
  let mut renderer = Renderer::default();
  let app = app();
  for size in [(0, 0), (0, 4), (10, 0)] {
    renderer
      .draw_frame(&mut UnexpectedOutput, app.frame(size), |_, _, _| {
        panic!("an empty viewport must not redraw a line");
      })
      .unwrap();
  }

  for size in [(1, 1), (2, 2), (120, 10)] {
    let lines = render(&mut renderer, app.frame(size));
    assert_eq!(row_numbers(&lines), (0..size.1).collect::<Vec<_>>());
  }
}

#[test]
fn drawing_errors_do_not_advance_the_cached_frame() {
  let mut renderer = Renderer::default();
  let mut app = app();
  let previous = app.frame((120, 10));
  render(&mut renderer, previous.clone());
  app.selected = 2;
  let next = app.frame((120, 10));

  let error = renderer
    .draw_frame(&mut Vec::new(), next.clone(), |_, _, _| {
      Err(io::Error::other("line drawing failed"))
    })
    .unwrap_err();

  assert_eq!(error.to_string(), "line drawing failed");
  assert_eq!(renderer.previous.as_ref(), Some(&previous));
  assert_eq!(row_numbers(&render(&mut renderer, next)), vec![3, 4]);
}

struct FailingOutput {
  fail_write: bool,
}

impl Write for FailingOutput {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    if self.fail_write {
      Err(io::Error::other("frame write failed"))
    } else {
      Ok(bytes.len())
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    Err(io::Error::other("frame flush failed"))
  }
}

#[test]
fn output_errors_do_not_advance_the_cached_frame() {
  for fail_write in [true, false] {
    let mut renderer = Renderer::default();
    let mut app = app();
    let previous = app.frame((120, 10));
    render(&mut renderer, previous.clone());
    app.selected = 2;
    let next = app.frame((120, 10));

    let error = renderer
      .draw_frame(
        &mut FailingOutput { fail_write },
        next.clone(),
        |output, _, line| output.write_all(line.text.as_bytes()),
      )
      .unwrap_err();

    assert_eq!(
      error.to_string(),
      if fail_write {
        "frame write failed"
      } else {
        "frame flush failed"
      }
    );
    assert_eq!(renderer.previous.as_ref(), Some(&previous));
    assert_eq!(row_numbers(&render(&mut renderer, next)), vec![3, 4]);
  }
}
