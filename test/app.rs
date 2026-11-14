use std::time::UNIX_EPOCH;

use crossterm::style::Color;

use super::render::{
  MAX_ENTRY_LINE_WIDTH, TIMESTAMP_WIDTH, format_entry_line, format_timestamp, synchronized_frame,
};
use super::*;

const TEST_NARROW_LINE_WIDTH: usize = 32;
const TEST_WIDE_LINE_WIDTH: usize = 80;
const TEST_NARROW_NAME_COLUMN_WIDTH: usize = 10;
const TEST_WIDE_NAME_COLUMN_WIDTH: usize = 24;
const TEST_TINY_LINE_WIDTH: usize = 20;

pub(super) fn app_with_entries(
  current_dir: PathBuf,
  entries: Vec<DirectoryEntry>,
  selected: usize,
) -> App {
  let visible_indices = (0..entries.len()).collect();
  App {
    current_dir,
    show_files: false,
    entries,
    visible_indices,
    selected,
    filter_query: String::new(),
    filter_kind: FilterKind::default(),
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  }
}

fn finish_scan(app: &mut App) -> bool {
  let mut changed = false;
  while app.scan.is_some() {
    changed |= app.poll_scan_events();
    std::thread::yield_now();
  }
  changed
}

fn temporary_cache_root(label: &str) -> PathBuf {
  let suffix = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_nanos();
  std::env::temp_dir().join(format!("fast-app-{label}-{}-{suffix}", std::process::id()))
}

#[test]
fn idle_loop_redraws_only_for_display_changes() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  let key = |code| Some(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
  let mut events = [
    None,
    key(KeyCode::Char('x')),
    key(KeyCode::Down),
    None,
    key(KeyCode::Down),
    Some(Event::Resize(100, 30)),
    key(KeyCode::Esc),
  ]
  .into_iter();
  let mut frames = Vec::new();
  let mut scan_active = Vec::new();

  let action = app
    .run_with(
      &mut Vec::new(),
      |app, _| {
        frames.push(app.selected);
        Ok(())
      },
      |active| {
        scan_active.push(active);
        Ok(events.next().expect("event loop should exit"))
      },
    )
    .unwrap();

  assert!(matches!(action, ExitAction::Cancel));
  assert_eq!(frames, vec![0, 1, 1]);
  assert!(scan_active.iter().all(|active| !active));
}

#[derive(Default)]
struct FrameOutput {
  bytes: Vec<u8>,
  writes: Vec<usize>,
  flushed_at: Vec<usize>,
  fail_write: bool,
  fail_flush: bool,
}

impl Write for FrameOutput {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.writes.push(bytes.len());
    if self.fail_write {
      return Err(io::Error::other("frame write failed"));
    }
    self.bytes.extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    self.flushed_at.push(self.bytes.len());
    if self.fail_flush {
      Err(io::Error::other("frame flush failed"))
    } else {
      Ok(())
    }
  }
}

#[test]
fn synchronized_frame_wraps_all_contents_before_flushing() {
  let mut output = FrameOutput::default();

  synchronized_frame(&mut output, |frame| {
    assert_eq!(frame.as_slice(), b"\x1b[?2026h");
    frame.write_all(b"header")?;
    frame.write_all(b"listing")?;
    frame.write_all(b"footer")
  })
  .unwrap();

  assert_eq!(output.bytes, b"\x1b[?2026hheaderlistingfooter\x1b[?2026l");
  assert_eq!(output.writes, vec![output.bytes.len()]);
  assert_eq!(output.flushed_at, vec![output.bytes.len()]);
}

#[test]
fn synchronized_frame_buffers_large_frames_before_writing() {
  let mut output = FrameOutput::default();
  let listing = vec![b'x'; 64 * 1024];

  synchronized_frame(&mut output, |frame| {
    frame.write_all(b"header")?;
    frame.write_all(&listing)?;
    frame.flush()?;
    frame.write_all(b"footer")
  })
  .unwrap();

  let expected = [
    b"\x1b[?2026hheader".as_slice(),
    listing.as_slice(),
    b"footer\x1b[?2026l".as_slice(),
  ]
  .concat();
  assert_eq!(output.bytes, expected);
  assert_eq!(output.writes, vec![expected.len()]);
  assert_eq!(output.flushed_at, vec![expected.len()]);
}

#[test]
fn synchronized_frame_ends_and_propagates_drawing_errors() {
  let mut output = FrameOutput::default();

  let error = synchronized_frame(&mut output, |output| {
    output.write_all(b"partial frame")?;
    Err(io::Error::other("frame drawing failed"))
  })
  .unwrap_err();

  assert_eq!(error.to_string(), "frame drawing failed");
  assert_eq!(output.bytes, b"\x1b[?2026hpartial frame\x1b[?2026l");
  assert_eq!(output.writes, vec![output.bytes.len()]);
  assert_eq!(output.flushed_at, vec![output.bytes.len()]);
}

#[test]
fn synchronized_frame_propagates_flush_errors() {
  let mut output = FrameOutput {
    fail_flush: true,
    ..FrameOutput::default()
  };

  let error = synchronized_frame(&mut output, |output| output.write_all(b"frame")).unwrap_err();

  assert_eq!(error.to_string(), "frame flush failed");
  assert_eq!(output.bytes, b"\x1b[?2026hframe\x1b[?2026l");
  assert_eq!(output.writes, vec![output.bytes.len()]);
  assert_eq!(output.flushed_at, vec![output.bytes.len()]);
}

#[test]
fn synchronized_frame_propagates_write_errors() {
  let mut output = FrameOutput {
    fail_write: true,
    ..FrameOutput::default()
  };

  let error = synchronized_frame(&mut output, |frame| frame.write_all(b"frame")).unwrap_err();

  assert_eq!(error.to_string(), "frame write failed");
  assert!(output.bytes.is_empty());
  assert_eq!(output.writes, vec![b"\x1b[?2026hframe\x1b[?2026l".len()]);
  assert!(output.flushed_at.is_empty());
}

#[test]
fn terminal_events_redraw_only_when_the_display_may_change() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));

  app.selection.pending = Some(current_dir.join("pending"));
  assert!(app.handle_event(key(KeyCode::Up)).is_none());
  assert!(app.selection.pending.is_none());
  assert!(app.handle_event(key(KeyCode::Char('x'))).is_none());
  assert!(matches!(
    app.handle_event(key(KeyCode::Down)),
    Some(KeyAction::Redraw)
  ));
  assert!(app.handle_event(key(KeyCode::Down)).is_none());
  assert!(app.handle_event(key(KeyCode::Enter)).is_none());
  assert!(matches!(
    app.handle_event(Event::Resize(100, 30)),
    Some(KeyAction::Redraw)
  ));
  assert!(matches!(
    app.handle_event(key(KeyCode::Char('y'))),
    Some(KeyAction::Copy(path)) if path == current_dir
  ));
  assert!(matches!(
    app.handle_event(key(KeyCode::Char('/'))),
    Some(KeyAction::Redraw)
  ));
  assert!(app.handle_event(key(KeyCode::Backspace)).is_none());
  assert!(matches!(
    app.handle_event(key(KeyCode::Char('a'))),
    Some(KeyAction::Redraw)
  ));
  assert!(
    app
      .handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('a'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
      )))
      .is_none()
  );
}

#[test]
fn scan_updates_request_redraw_and_stop_after_completion() {
  let current_dir = std::env::current_dir().unwrap();
  let mut app = app_with_entries(current_dir.clone(), Vec::new(), 0);
  app.status = ScanStatus::Refreshing;
  app
    .timestamps
    .insert(current_dir.clone(), TimestampState::Pending);
  app.scan = Some(ScanHandle::start_metadata(vec![current_dir.clone()]).unwrap());

  assert!(finish_scan(&mut app));
  assert!(matches!(app.status, ScanStatus::Ready));
  assert!(!matches!(
    app.timestamps.get(&current_dir),
    Some(TimestampState::Pending)
  ));
  assert!(!app.poll_scan_events());
}

#[test]
fn scan_errors_request_redraw_and_stop_after_completion() {
  let missing_dir = temporary_cache_root("missing-scan");
  let mut app = App::with_cache(missing_dir, None);

  assert!(finish_scan(&mut app));
  assert!(matches!(app.status, ScanStatus::Error(_)));
  assert!(!app.poll_scan_events());
}

#[test]
fn formats_local_timestamps_with_fixed_width() {
  let timestamp = format_timestamp(SystemTime::now()).expect("current time should format");

  assert_eq!(timestamp.len(), TIMESTAMP_WIDTH);
  assert_eq!(&timestamp[4..5], "-");
  assert_eq!(&timestamp[7..8], "-");
  assert_eq!(&timestamp[10..11], " ");
  assert_eq!(&timestamp[13..14], ":");
}

#[test]
fn entry_layout_truncates_names_without_mutating_the_source() {
  let name = "a-very-long-entry-name";
  let timestamp = "2026-09-16 12:34";

  let narrow = format_entry_line(
    "> ",
    name,
    timestamp,
    TEST_NARROW_LINE_WIDTH,
    Some(TEST_NARROW_NAME_COLUMN_WIDTH),
  );
  assert!(narrow.contains("..."));
  assert!(narrow.ends_with(timestamp));
  assert!(UnicodeWidthStr::width(narrow.as_str()) <= TEST_NARROW_LINE_WIDTH);

  let wide = format_entry_line(
    "  ",
    name,
    timestamp,
    TEST_WIDE_LINE_WIDTH,
    Some(TEST_WIDE_NAME_COLUMN_WIDTH),
  );
  assert!(wide.contains(name));
  assert!(wide.ends_with(timestamp));

  let too_narrow = format_entry_line("  ", name, timestamp, TEST_TINY_LINE_WIDTH, None);
  assert!(!too_narrow.contains(timestamp));
  assert!(too_narrow.contains("..."));

  let wide_name = format_entry_line(
    "  ",
    "wide-directory-name",
    timestamp,
    TEST_NARROW_LINE_WIDTH,
    Some(TEST_NARROW_NAME_COLUMN_WIDTH),
  );
  assert!(wide_name.ends_with(timestamp));
  assert!(UnicodeWidthStr::width(wide_name.as_str()) <= TEST_NARROW_LINE_WIDTH);

  let short = format_entry_line("  ", "src", timestamp, TEST_WIDE_LINE_WIDTH, Some(12));
  let medium = format_entry_line(
    "  ",
    "Cargo.toml",
    timestamp,
    TEST_WIDE_LINE_WIDTH,
    Some(12),
  );
  assert_eq!(short.find(timestamp), medium.find(timestamp));
}

#[test]
fn name_column_uses_the_total_line_width_limit() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(
    current_dir.clone(),
    vec![DirectoryEntry {
      name: "this-entry-name-is-deliberately-longer-than-fifty-eight-cells".to_owned(),
      path: current_dir.join("long-entry"),
      is_directory: true,
    }],
    0,
  );
  app.refresh_visible();

  assert_eq!(app.name_column_width(MAX_ENTRY_LINE_WIDTH), Some(58));
  assert_eq!(app.name_column_width(120), Some(58));
  assert_eq!(app.name_column_width(70), Some(48));
  assert_eq!(app.name_column_width(25), None);
}

#[test]
fn metadata_updates_replace_pending_state_by_path() {
  let path = PathBuf::from("/tmp/current/entry");
  let mut app = app_with_entries(
    PathBuf::from("/tmp/current"),
    vec![DirectoryEntry {
      name: "entry".to_owned(),
      path: path.clone(),
      is_directory: true,
    }],
    0,
  );
  app.status = ScanStatus::Refreshing;
  app.timestamps.insert(path.clone(), TimestampState::Pending);

  app.apply_scan_event(ScanEvent::MetadataChunk(vec![EntryMetadata {
    path: path.clone(),
    modified: Some(UNIX_EPOCH),
  }]));

  assert!(matches!(
    app.timestamps.get(&path),
    Some(TimestampState::Available(modified)) if *modified == UNIX_EPOCH
  ));
}

#[test]
fn copy_path_emits_an_osc52_clipboard_command() {
  let mut output = Vec::new();

  copy_path(&mut output, Path::new("/tmp/current/target")).unwrap();

  assert_eq!(output, b"\x1b]52;c;L3RtcC9jdXJyZW50L3RhcmdldA==\x1b\\");
}

#[cfg(unix)]
#[test]
fn copy_path_ignores_a_non_utf8_path() {
  use std::{ffi::OsString, os::unix::ffi::OsStringExt};

  let path = PathBuf::from(OsString::from_vec(vec![b'/', 0xff]));
  let mut output = Vec::new();

  copy_path(&mut output, &path).unwrap();

  assert!(output.is_empty());
}

#[test]
fn y_copies_the_highlighted_file_path_without_exiting() {
  let current_dir = PathBuf::from("/tmp/current");
  let file_path = current_dir.join("file.txt");
  let mut app = app_with_entries(
    current_dir,
    vec![DirectoryEntry {
      name: "file.txt".to_owned(),
      path: file_path.clone(),
      is_directory: false,
    }],
    0,
  );
  app.show_files = true;

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

  assert!(matches!(action, Some(KeyAction::Copy(path)) if path == file_path));
}

#[test]
fn y_copies_navigation_entry_paths() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  let parent = current_dir.parent().unwrap().to_path_buf();

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
  assert!(matches!(action, Some(KeyAction::Copy(path)) if path == parent));

  app.selected = 1;
  let action = app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
  assert!(matches!(action, Some(KeyAction::Copy(path)) if path == current_dir));
}

#[test]
fn y_in_filter_mode_remains_query_input() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(
    current_dir.clone(),
    vec![DirectoryEntry {
      name: "target".to_owned(),
      path: current_dir.join("target"),
      is_directory: true,
    }],
    0,
  );
  app.filter_mode = true;

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

  assert!(matches!(action, Some(KeyAction::Redraw)));
  assert_eq!(app.filter_query, "y");
}

#[test]
fn y_does_nothing_without_a_highlighted_entry() {
  let mut app = app_with_entries(PathBuf::from("/tmp/current"), Vec::new(), 0);

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

  assert!(action.is_none());
}

#[test]
fn q_selects_the_highlighted_entry() {
  let current_dir = PathBuf::from("/tmp/current");
  let selected_path = current_dir.join("target");
  let mut app = App {
    current_dir,
    show_files: false,
    entries: vec![DirectoryEntry {
      name: "target".to_owned(),
      path: selected_path.clone(),
      is_directory: true,
    }],
    visible_indices: vec![0],
    selected: 0,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
  match action {
    Some(KeyAction::Exit(ExitAction::Select(path))) => assert_eq!(path, selected_path),
    _ => panic!("q should select the highlighted entry"),
  }
}

#[test]
fn q_on_a_file_selects_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(
    current_dir.clone(),
    vec![DirectoryEntry {
      name: "file.txt".to_owned(),
      path: current_dir.join("file.txt"),
      is_directory: false,
    }],
    0,
  );
  app.show_files = true;

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

  assert!(matches!(
    action,
    Some(KeyAction::Exit(ExitAction::Select(path))) if path == current_dir
  ));
}

#[test]
fn opening_a_file_with_enter_right_or_l_is_a_no_op() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(
    current_dir.clone(),
    vec![DirectoryEntry {
      name: "file.txt".to_owned(),
      path: current_dir.join("file.txt"),
      is_directory: false,
    }],
    0,
  );
  app.show_files = true;

  for key_code in [KeyCode::Enter, KeyCode::Right, KeyCode::Char('l')] {
    app.handle_key(KeyEvent::new(key_code, KeyModifiers::NONE));
    assert_eq!(app.current_dir, current_dir);
    assert!(app.scan.is_none());
    assert!(matches!(app.status, ScanStatus::Ready));
  }
}

#[test]
fn uppercase_f_replaces_the_active_listing_mode() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir, Vec::new(), 0);

  app.handle_key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE));
  assert!(app.show_files);
  assert!(matches!(app.status, ScanStatus::Indexing));
  app.stop_scan();

  app.handle_key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE));
  assert!(!app.show_files);
  assert!(matches!(app.status, ScanStatus::Indexing));
  app.stop_scan();
}

#[test]
fn h_remains_the_parent_navigation_shortcut() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir, Vec::new(), 0);

  app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));

  assert_eq!(app.current_dir, PathBuf::from("/tmp"));
  app.stop_scan();
}

#[test]
fn mixed_listings_without_history_default_to_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);
  app.show_files = true;

  app.apply_scan_event(ScanEvent::Chunk(vec![
    DirectoryEntry {
      name: "a-file".to_owned(),
      path: current_dir.join("a-file"),
      is_directory: false,
    },
    DirectoryEntry {
      name: "z-directory".to_owned(),
      path: current_dir.join("z-directory"),
      is_directory: true,
    },
  ]));

  assert_eq!(app.selected_path(), Some(current_dir));
}

#[test]
fn directories_are_sorted_before_files_and_files_get_a_header_row() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut entries = navigation_entries(&current_dir);
  entries.extend([
    DirectoryEntry {
      name: "z-file".to_owned(),
      path: current_dir.join("z-file"),
      is_directory: false,
    },
    DirectoryEntry {
      name: "a-directory".to_owned(),
      path: current_dir.join("a-directory"),
      is_directory: true,
    },
  ]);
  let mut app = app_with_entries(current_dir, entries, 0);
  app.show_files = true;
  app.sort_entries();
  app.refresh_visible();

  let names = app
    .visible_indices
    .iter()
    .map(|&index| app.entries[index].name.as_str())
    .collect::<Vec<_>>();
  assert_eq!(names, vec!["..", ".", "a-directory", "z-file"]);
  assert_eq!(app.files_position(), Some(3));
  assert_eq!(app.selected_row(app.files_position()), 0);

  app.selected = 3;
  assert_eq!(app.selected_row(app.files_position()), 4);
  assert_eq!(
    app.scroll_start(3, app.selected_row(app.files_position())),
    2
  );
  assert_eq!(App::entry_color(&app.entries[3]), Color::DarkGrey);
  assert_eq!(App::entry_color(&app.entries[2]), Color::White);
}

#[test]
fn file_only_listings_default_to_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);
  app.show_files = true;

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "file.txt".to_owned(),
    path: current_dir.join("file.txt"),
    is_directory: false,
  }]));
  app.apply_scan_event(ScanEvent::Finished { cacheable: true });

  assert_eq!(app.selected_path(), Some(current_dir));
}

#[test]
fn navigation_entries_include_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");

  assert_eq!(
    navigation_entries(&current_dir),
    vec![
      DirectoryEntry {
        name: "..".to_owned(),
        path: PathBuf::from("/tmp"),
        is_directory: true,
      },
      DirectoryEntry {
        name: ".".to_owned(),
        path: current_dir,
        is_directory: true,
      },
    ]
  );
}

#[test]
fn root_navigation_entries_only_include_the_current_directory() {
  let entries = navigation_entries(Path::new("/"));

  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].name, ".");
  assert_eq!(entries[0].path, PathBuf::from("/"));
}

#[test]
fn q_can_select_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: navigation_entries(&current_dir),
    visible_indices: vec![0, 1],
    selected: 1,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };

  let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

  assert!(matches!(
    action,
    Some(KeyAction::Exit(ExitAction::Select(path))) if path == current_dir
  ));
}

#[test]
fn opening_the_current_directory_is_a_no_op() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: navigation_entries(&current_dir),
    visible_indices: vec![0, 1],
    selected: 1,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };

  app.open_selected();

  assert_eq!(app.current_dir, current_dir);
  assert!(app.scan.is_none());
  assert!(matches!(app.status, ScanStatus::Ready));
}

#[test]
fn filter_input_updates_visible_entries() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: vec![
      DirectoryEntry {
        name: "..".to_owned(),
        path: PathBuf::from("/tmp"),
        is_directory: true,
      },
      DirectoryEntry {
        name: ".".to_owned(),
        path: current_dir.clone(),
        is_directory: true,
      },
      DirectoryEntry {
        name: "Target".to_owned(),
        path: current_dir.join("Target"),
        is_directory: true,
      },
      DirectoryEntry {
        name: "logs".to_owned(),
        path: current_dir.join("logs"),
        is_directory: true,
      },
    ],
    visible_indices: Vec::new(),
    selected: 0,
    filter_query: String::new(),
    filter_kind: FilterKind::default(),
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.refresh_visible();

  app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
  app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

  let visible_names = app
    .visible_indices
    .iter()
    .map(|&index| app.entries[index].name.as_str())
    .collect::<Vec<_>>();
  assert!(app.filter_mode);
  assert_eq!(app.filter_query, "t");
  assert_eq!(visible_names, vec!["..", ".", "Target"]);
}

#[test]
fn empty_results_cannot_select_the_current_directory() {
  let mut app = App {
    current_dir: PathBuf::from("/"),
    show_files: false,
    entries: vec![DirectoryEntry {
      name: "target".to_owned(),
      path: PathBuf::from("/target"),
      is_directory: true,
    }],
    visible_indices: Vec::new(),
    selected: 0,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.refresh_visible();
  app.set_filter_query("missing".to_owned());

  assert!(app.visible_indices.is_empty());
  assert!(
    app
      .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
      .is_none()
  );
}

#[test]
fn filter_update_preserves_the_selected_path() {
  let current_dir = PathBuf::from("/tmp/current");
  let selected_path = current_dir.join("logs");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: vec![
      DirectoryEntry {
        name: "target".to_owned(),
        path: current_dir.join("target"),
        is_directory: true,
      },
      DirectoryEntry {
        name: "logs".to_owned(),
        path: selected_path.clone(),
        is_directory: true,
      },
    ],
    visible_indices: Vec::new(),
    selected: 1,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.refresh_visible();
  app.set_filter_query("log".to_owned());

  assert_eq!(app.selected_path(), Some(selected_path));
  assert_eq!(app.selected, 0);
}

#[test]
fn filter_keys_edit_and_clear_the_query() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: vec![
      DirectoryEntry {
        name: "target".to_owned(),
        path: current_dir.join("target"),
        is_directory: true,
      },
      DirectoryEntry {
        name: "logs".to_owned(),
        path: current_dir.join("logs"),
        is_directory: true,
      },
    ],
    visible_indices: Vec::new(),
    selected: 0,
    filter_query: String::new(),
    filter_kind: FilterKind::default(),
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.refresh_visible();

  app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
  app.handle_key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
  app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
  app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
  app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

  assert!(app.filter_mode);
  assert_eq!(app.filter_query, "T");
  assert_eq!(app.filter_kind, FilterKind::Substring);
  assert_eq!(app.visible_indices, vec![0]);
  app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
  assert_eq!(app.filter_kind, FilterKind::Fuzzy);
  app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
  assert!(!app.filter_mode);
  assert_eq!(app.filter_query, "T");

  assert!(matches!(
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    Some(KeyAction::Redraw)
  ));
  assert!(app.filter_query.is_empty());
  assert!(matches!(
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    Some(KeyAction::Exit(ExitAction::Cancel))
  ));
}

#[test]
fn scan_chunks_keep_unfiltered_entries_and_selected_path() {
  let current_dir = PathBuf::from("/tmp/current");
  let selected_path = current_dir.join("alpha");
  let mut app = App {
    current_dir: current_dir.clone(),
    show_files: false,
    entries: vec![
      DirectoryEntry {
        name: "..".to_owned(),
        path: PathBuf::from("/tmp"),
        is_directory: true,
      },
      DirectoryEntry {
        name: ".".to_owned(),
        path: current_dir.clone(),
        is_directory: true,
      },
      DirectoryEntry {
        name: "alpha".to_owned(),
        path: selected_path.clone(),
        is_directory: true,
      },
    ],
    visible_indices: Vec::new(),
    selected: 2,
    filter_query: "a".to_owned(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: None,
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Indexing,
  };
  app.refresh_visible();

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "beta".to_owned(),
    path: current_dir.join("beta"),
    is_directory: true,
  }]));

  assert_eq!(app.entries.len(), 4);
  assert_eq!(app.visible_indices, vec![0, 1, 2, 3]);
  assert_eq!(app.selected_path(), Some(selected_path));
}

#[test]
fn unvisited_directory_stays_on_the_current_directory_as_scan_chunks_arrive() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);

  app.apply_scan_event(ScanEvent::Chunk(vec![
    DirectoryEntry {
      name: "beta".to_owned(),
      path: current_dir.join("beta"),
      is_directory: true,
    },
    DirectoryEntry {
      name: "alpha".to_owned(),
      path: current_dir.join("alpha"),
      is_directory: true,
    },
  ]));

  assert_eq!(app.selected_path(), Some(current_dir.clone()));
  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "gamma".to_owned(),
    path: current_dir.join("gamma"),
    is_directory: true,
  }]));
  assert_eq!(app.selected_path(), Some(current_dir.clone()));
  app.apply_scan_event(ScanEvent::Finished { cacheable: true });
  assert_eq!(app.selected_path(), Some(current_dir));
  assert!(app.selection.pending.is_none());
}

#[test]
fn root_directory_defaults_to_the_current_directory() {
  let current_dir = PathBuf::from("/");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "alpha".to_owned(),
    path: current_dir.join("alpha"),
    is_directory: true,
  }]));

  assert_eq!(app.selected_path(), Some(current_dir));
}

#[test]
fn empty_directory_keeps_the_current_directory_selected() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);
  app.apply_scan_event(ScanEvent::Finished { cacheable: true });
  assert_eq!(app.selected_path(), Some(current_dir.clone()));

  let root = PathBuf::from("/");
  let mut app = app_with_entries(root.clone(), navigation_entries(&root), 0);
  app.apply_scan_event(ScanEvent::Finished { cacheable: true });
  assert_eq!(app.selected_path(), Some(root));
}

#[test]
fn manual_selection_movement_overrides_pending_restoration() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  app.selection.pending = Some(current_dir.join("alpha"));

  app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "alpha".to_owned(),
    path: current_dir.join("alpha"),
    is_directory: true,
  }]));

  assert_eq!(app.selected_path(), Some(current_dir));
  assert!(app.selection.pending.is_none());
}

#[test]
fn manual_home_and_end_override_pending_restoration() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);
  app.selection.pending = Some(current_dir.join("alpha"));
  app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "alpha".to_owned(),
    path: current_dir.join("alpha"),
    is_directory: true,
  }]));
  assert_eq!(app.selected_path(), Some(PathBuf::from("/tmp")));

  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  app.selection.pending = Some(current_dir.join("alpha"));
  app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "alpha".to_owned(),
    path: current_dir.join("alpha"),
    is_directory: true,
  }]));
  assert_eq!(app.selected_path(), Some(current_dir));
}

#[test]
fn returning_to_parent_restores_the_child_entry() {
  let current_dir = PathBuf::from("/tmp/current");
  let child = current_dir.join("child");
  let mut app = app_with_entries(
    current_dir.clone(),
    vec![
      DirectoryEntry {
        name: "..".to_owned(),
        path: PathBuf::from("/tmp"),
        is_directory: true,
      },
      DirectoryEntry {
        name: ".".to_owned(),
        path: current_dir.clone(),
        is_directory: true,
      },
      DirectoryEntry {
        name: "child".to_owned(),
        path: child.clone(),
        is_directory: true,
      },
    ],
    2,
  );

  app.open_selected();
  assert_eq!(app.current_dir, child);
  assert_eq!(app.selected_path(), Some(child.clone()));
  app.stop_scan();

  app.open_parent();
  app.stop_scan();
  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "child".to_owned(),
    path: child.clone(),
    is_directory: true,
  }]));

  assert_eq!(app.selected_path(), Some(child));
}

#[test]
fn chunked_scan_restores_remembered_selection_when_entry_arrives() {
  let current_dir = PathBuf::from("/tmp/current");
  let selected_path = current_dir.join("target");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  app
    .selection
    .remembered
    .insert(current_dir.clone(), selected_path.clone());
  app.start_scan(false);
  app.stop_scan();

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "other".to_owned(),
    path: PathBuf::from("/tmp/current/other"),
    is_directory: true,
  }]));
  assert_eq!(app.selected_path(), Some(current_dir.clone()));

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "target".to_owned(),
    path: selected_path.clone(),
    is_directory: true,
  }]));
  assert_eq!(app.selected_path(), Some(selected_path));
}

#[test]
fn missing_remembered_selection_falls_back_to_the_current_directory() {
  let current_dir = PathBuf::from("/tmp/current");
  let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
  app
    .selection
    .remembered
    .insert(current_dir.clone(), current_dir.join("missing"));
  app.start_scan(false);
  app.stop_scan();

  app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
    name: "other".to_owned(),
    path: current_dir.join("other"),
    is_directory: true,
  }]));
  app.apply_scan_event(ScanEvent::Finished { cacheable: true });

  assert_eq!(app.selected_path(), Some(current_dir));
  assert!(app.selected < app.visible_indices.len());
}

#[test]
fn uses_a_valid_cache_before_starting_a_scan() {
  let root = temporary_cache_root("cache");
  let directory = root.join("workspace");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&directory).unwrap();
  let child = directory.join("child");
  std::fs::create_dir(&child).unwrap();
  let entries = vec![DirectoryEntry {
    name: "child".to_owned(),
    path: child.clone(),
    is_directory: true,
  }];
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  assert!(
    cache
      .store_if_unchanged(&directory, &fingerprint, &entries)
      .unwrap()
  );

  let mut app = App::with_cache(directory.clone(), Some(cache));

  assert_eq!(app.filter_kind, FilterKind::Fuzzy);
  assert!(matches!(app.status, ScanStatus::Refreshing));
  assert!(app.scan.is_some());
  assert_eq!(app.entries[0].name, "..");
  assert_eq!(app.entries[1].name, ".");
  assert_eq!(app.selected_path(), Some(directory.clone()));
  assert_eq!(app.discovered_count(), 1);
  assert_eq!(
    app
      .entries
      .iter()
      .filter(|entry| entry.name == "child")
      .count(),
    1
  );
  assert!(matches!(
    app.timestamps.get(&child),
    Some(TimestampState::Pending)
  ));

  finish_scan(&mut app);
  assert!(matches!(app.status, ScanStatus::Ready));
  assert!(matches!(
    app.timestamps.get(&child),
    Some(TimestampState::Available(_))
  ));

  app.selected = app
    .visible_indices
    .iter()
    .position(|&index| app.entries[index].path == child)
    .unwrap();
  app.remember_selection();
  app.start_scan(false);
  finish_scan(&mut app);

  assert_eq!(app.selected_path(), Some(child.clone()));

  app.filter_kind = FilterKind::Substring;
  app.filter_query = "child".to_owned();
  app.start_scan(false);
  finish_scan(&mut app);

  assert_eq!(app.filter_kind, FilterKind::Fuzzy);
  assert!(app.filter_query.is_empty());
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rescan_bypasses_a_valid_cache() {
  let root = temporary_cache_root("force-rescan");
  let directory = root.join("workspace");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&directory).unwrap();
  let fresh = directory.join("fresh");
  std::fs::create_dir(&fresh).unwrap();
  let stale = directory.join("stale");
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  assert!(
    cache
      .store_if_unchanged(
        &directory,
        &fingerprint,
        &[DirectoryEntry {
          name: "stale".to_owned(),
          path: stale,
          is_directory: true,
        }],
      )
      .unwrap()
  );

  let mut app = App::with_cache(directory.clone(), Some(cache.clone()));
  assert!(app.entries.iter().any(|entry| entry.name == "stale"));

  app.rescan();
  assert!(matches!(app.status, ScanStatus::Indexing));
  finish_scan(&mut app);

  assert!(app.entries.iter().any(|entry| entry.path == fresh));
  assert!(!app.entries.iter().any(|entry| entry.name == "stale"));
  assert_eq!(
    cache.load(&directory).unwrap(),
    Some(vec![DirectoryEntry {
      name: "fresh".to_owned(),
      path: fresh,
      is_directory: true,
    }])
  );
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn incomplete_scan_does_not_replace_the_cache() {
  let root = temporary_cache_root("incomplete-scan");
  let directory = root.join("workspace");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&directory).unwrap();
  let cached_path = directory.join("cached");
  std::fs::create_dir(&cached_path).unwrap();
  let cached_entries = vec![DirectoryEntry {
    name: "cached".to_owned(),
    path: cached_path,
    is_directory: true,
  }];
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  assert!(
    cache
      .store_if_unchanged(&directory, &fingerprint, &cached_entries)
      .unwrap()
  );

  let mut app = App::with_cache(directory.clone(), Some(cache.clone()));
  app.stop_scan();
  app.entries = navigation_entries(&directory);
  app.entries.push(DirectoryEntry {
    name: "replacement".to_owned(),
    path: directory.join("replacement"),
    is_directory: true,
  });
  app.scan_fingerprint = Some(fingerprint);
  app.persist_scan(false);

  assert_eq!(cache.load(&directory).unwrap(), Some(cached_entries));
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn does_not_persist_navigation_entries_in_the_cache() {
  let root = temporary_cache_root("navigation-cache");
  let directory = root.join("workspace");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&directory).unwrap();
  let child = directory.join("child");
  std::fs::create_dir(&child).unwrap();
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  let mut app = App {
    current_dir: directory.clone(),
    show_files: false,
    entries: navigation_entries(&directory),
    visible_indices: Vec::new(),
    selected: 0,
    filter_query: String::new(),
    filter_kind: FilterKind::Substring,
    filter_mode: false,
    cache: Some(cache.clone()),
    scan: None,
    scan_fingerprint: Some(fingerprint),
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.entries.push(DirectoryEntry {
    name: "child".to_owned(),
    path: child.clone(),
    is_directory: true,
  });

  app.persist_scan(true);

  assert_eq!(
    cache.load(&directory).unwrap(),
    Some(vec![DirectoryEntry {
      name: "child".to_owned(),
      path: child,
      is_directory: true,
    }])
  );
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn file_visible_scan_does_not_write_to_the_directory_cache() {
  let root = temporary_cache_root("file-cache");
  let directory = root.join("workspace");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&directory).unwrap();
  let file = directory.join("file.txt");
  std::fs::write(&file, b"content").unwrap();
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  let mut app = App {
    current_dir: directory.clone(),
    show_files: true,
    entries: navigation_entries(&directory),
    visible_indices: vec![0, 1],
    selected: 1,
    filter_query: String::new(),
    filter_kind: FilterKind::default(),
    filter_mode: false,
    cache: Some(cache.clone()),
    scan: None,
    scan_fingerprint: Some(fingerprint),
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };
  app.entries.push(DirectoryEntry {
    name: "file.txt".to_owned(),
    path: file,
    is_directory: false,
  });

  app.persist_scan(true);

  assert_eq!(cache.load(&directory).unwrap(), None);
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn file_visible_scan_ignores_directory_cache() {
  let root = temporary_cache_root("file-cache-read");
  let directory = root.join("workspace");
  let child = directory.join("child");
  let cache = DirectoryCache::new(root.join("cache"));
  std::fs::create_dir_all(&child).unwrap();
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  assert!(
    cache
      .store_if_unchanged(
        &directory,
        &fingerprint,
        &[DirectoryEntry {
          name: "child".to_owned(),
          path: child,
          is_directory: true,
        }],
      )
      .unwrap()
  );

  let mut app = App {
    current_dir: directory.clone(),
    show_files: true,
    entries: navigation_entries(&directory),
    visible_indices: vec![0, 1],
    selected: 1,
    filter_query: String::new(),
    filter_kind: FilterKind::default(),
    filter_mode: false,
    cache: Some(cache),
    scan: None,
    scan_fingerprint: None,
    timestamps: HashMap::new(),
    max_visible_name_width: 0,
    selection: SelectionState::default(),
    status: ScanStatus::Ready,
  };

  app.start_scan(false);

  assert!(app.scan.is_some());
  app.stop_scan();
  let _ = std::fs::remove_dir_all(root);
}
