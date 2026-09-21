use std::{
  collections::HashMap,
  io::{self, Stdout, Write},
  path::{Path, PathBuf},
  time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crossterm::{
  clipboard::CopyToClipboard,
  cursor::MoveTo,
  event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
  execute, queue,
  style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
  terminal::{self, Clear, ClearType},
};

use crate::{
  cache::{DirectoryCache, DirectoryFingerprint},
  filter::{FilterKind, fuzzy_indices, matching_indices},
  scan::{DirectoryEntry, EntryMetadata, ScanEvent, ScanHandle, modified_time},
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MARKER_WIDTH: usize = 2;
const TIMESTAMP_WIDTH: usize = 16;
const TIMESTAMP_GAP: usize = 4;
const MIN_NAME_WIDTH: usize = 4;
const MAX_ENTRY_LINE_WIDTH: usize = 80;
const TIMESTAMP_FORMAT: &[time::format_description::FormatItem<'static>] =
  time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]");

pub(crate) struct App {
  current_dir: PathBuf,
  show_files: bool,
  entries: Vec<DirectoryEntry>,
  visible_indices: Vec<usize>,
  selected: usize,
  filter_query: String,
  filter_kind: FilterKind,
  filter_mode: bool,
  cache: Option<DirectoryCache>,
  scan: Option<ScanHandle>,
  scan_fingerprint: Option<DirectoryFingerprint>,
  timestamps: HashMap<PathBuf, TimestampState>,
  max_visible_name_width: usize,
  selection: SelectionState,
  status: ScanStatus,
}

enum ScanStatus {
  Indexing,
  Refreshing,
  Ready,
  Error(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TimestampState {
  Pending,
  Available(SystemTime),
  Unavailable,
}

impl TimestampState {
  fn from_modified(modified: Option<SystemTime>) -> Self {
    modified.map_or(Self::Unavailable, Self::Available)
  }
}

#[derive(Default)]
struct SelectionState {
  remembered: HashMap<PathBuf, PathBuf>,
  pending: Option<PathBuf>,
}

pub(crate) enum ExitAction {
  Select(PathBuf),
  Cancel,
}

enum KeyAction {
  Exit(ExitAction),
  Copy(PathBuf),
}

impl App {
  pub(crate) fn new(current_dir: PathBuf) -> Self {
    Self::with_cache(current_dir, DirectoryCache::system())
  }

  fn with_cache(current_dir: PathBuf, cache: Option<DirectoryCache>) -> Self {
    let mut app = Self {
      current_dir,
      show_files: false,
      entries: Vec::new(),
      visible_indices: Vec::new(),
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::default(),
      filter_mode: false,
      cache,
      scan: None,
      scan_fingerprint: None,
      timestamps: HashMap::new(),
      max_visible_name_width: 0,
      selection: SelectionState::default(),
      status: ScanStatus::Indexing,
    };
    app.start_scan(false);
    app
  }

  pub(crate) fn run(&mut self, output: &mut Stdout) -> io::Result<ExitAction> {
    let action = loop {
      self.poll_scan_events();
      self.draw(output)?;

      if event::poll(EVENT_POLL_INTERVAL)?
        && let Event::Key(key) = event::read()?
      {
        match self.handle_key(key) {
          Some(KeyAction::Exit(action)) => break action,
          Some(KeyAction::Copy(path)) => copy_path(output, &path)?,
          None => {}
        }
      }
    };

    self.stop_scan();
    Ok(action)
  }

  fn handle_key(&mut self, key: KeyEvent) -> Option<KeyAction> {
    if key.kind != KeyEventKind::Press {
      return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
      return Some(KeyAction::Exit(ExitAction::Cancel));
    }

    if self.filter_mode {
      return self.handle_filter_key(key);
    }

    match key.code {
      KeyCode::Esc => {
        if self.filter_query.is_empty() {
          Some(KeyAction::Exit(ExitAction::Cancel))
        } else {
          self.set_filter_query(String::new());
          None
        }
      }
      KeyCode::Char('/') => {
        self.filter_mode = true;
        None
      }
      KeyCode::Char('F') => {
        self.toggle_files();
        None
      }
      KeyCode::Char('q') | KeyCode::Char('Q') => self
        .selected_result_path()
        .map(|path| KeyAction::Exit(ExitAction::Select(path))),
      KeyCode::Char('y') => self.selected_path().map(KeyAction::Copy),
      KeyCode::Up | KeyCode::Char('k') => {
        self.move_selection(-1);
        None
      }
      KeyCode::Down | KeyCode::Char('j') => {
        self.move_selection(1);
        None
      }
      KeyCode::Home | KeyCode::Char('g') => {
        self.cancel_pending_selection();
        self.selected = 0;
        None
      }
      KeyCode::End | KeyCode::Char('G') => {
        self.cancel_pending_selection();
        self.selected = self.visible_indices.len().saturating_sub(1);
        None
      }
      KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
        self.open_selected();
        None
      }
      KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
        self.open_parent();
        None
      }
      KeyCode::Char('r') | KeyCode::Char('R') => {
        self.rescan();
        None
      }
      _ => None,
    }
  }

  fn handle_filter_key(&mut self, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
      KeyCode::Esc => {
        self.filter_mode = false;
        self.set_filter_query(String::new());
        None
      }
      KeyCode::Enter => {
        self.filter_mode = false;
        None
      }
      KeyCode::Tab => {
        let selected_path = self.selected_path();
        self.filter_kind = self.filter_kind.toggle();
        self.refresh_visible();
        self.restore_selection(selected_path.as_deref());
        None
      }
      KeyCode::Backspace => {
        let mut query = self.filter_query.clone();
        query.pop();
        self.set_filter_query(query);
        None
      }
      KeyCode::Up => {
        self.move_selection(-1);
        None
      }
      KeyCode::Down => {
        self.move_selection(1);
        None
      }
      KeyCode::Home => {
        self.cancel_pending_selection();
        self.selected = 0;
        None
      }
      KeyCode::End => {
        self.cancel_pending_selection();
        self.selected = self.visible_indices.len().saturating_sub(1);
        None
      }
      KeyCode::Char(character)
        if !key
          .modifiers
          .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
      {
        let mut query = self.filter_query.clone();
        query.push(character);
        self.set_filter_query(query);
        None
      }
      _ => None,
    }
  }

  fn poll_scan_events(&mut self) {
    let mut events = Vec::new();
    let mut disconnected = false;
    if let Some(scan) = self.scan.as_ref() {
      loop {
        match scan.try_recv() {
          Ok(event) => events.push(event),
          Err(std::sync::mpsc::TryRecvError::Empty) => break,
          Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            disconnected = true;
            break;
          }
        }
      }
    }

    for event in events {
      self.apply_scan_event(event);
    }
    if disconnected && self.scan.is_some() {
      self.scan = None;
      self.mark_pending_timestamps_unavailable();
      self.status = ScanStatus::Error("directory scanner stopped unexpectedly".to_owned());
    }
  }

  fn apply_scan_event(&mut self, event: ScanEvent) {
    match event {
      ScanEvent::Chunk(entries) => self.apply_entries(entries),
      ScanEvent::MetadataChunk(updates) => self.apply_metadata(updates),
      ScanEvent::Finished { cacheable } => {
        self.finish_pending_selection();
        self.persist_scan(cacheable);
        self.status = ScanStatus::Ready;
        self.scan = None;
        self.scan_fingerprint = None;
      }
      ScanEvent::Error(error) => {
        self.selection.pending = None;
        self.mark_pending_timestamps_unavailable();
        self.status = ScanStatus::Error(error);
        self.scan = None;
        self.scan_fingerprint = None;
      }
    }
  }

  fn apply_entries(&mut self, entries: Vec<DirectoryEntry>) {
    let selected_path = self.selected_path();
    let appended_max_name_width = if self.filter_query.is_empty() {
      entries
        .iter()
        .map(|entry| UnicodeWidthStr::width(entry.name.as_str()))
        .max()
        .unwrap_or(0)
    } else {
      0
    };
    for entry in &entries {
      self
        .timestamps
        .insert(entry.path.clone(), TimestampState::Pending);
    }
    self.entries.extend(entries);
    self.sort_entries();
    if self.filter_query.is_empty() {
      self.visible_indices = (0..self.entries.len()).collect();
      self.max_visible_name_width = self.max_visible_name_width.max(appended_max_name_width);
      self.selected = self
        .selected
        .min(self.visible_indices.len().saturating_sub(1));
    } else {
      self.refresh_visible();
    }
    if !self.try_restore_pending_selection() {
      self.restore_selection(selected_path.as_deref());
    }
  }

  fn apply_metadata(&mut self, updates: Vec<EntryMetadata>) {
    for update in updates {
      self
        .timestamps
        .insert(update.path, TimestampState::from_modified(update.modified));
    }
  }

  fn mark_pending_timestamps_unavailable(&mut self) {
    for state in self.timestamps.values_mut() {
      if matches!(state, TimestampState::Pending) {
        *state = TimestampState::Unavailable;
      }
    }
  }

  fn start_scan(&mut self, bypass_cache: bool) {
    self.stop_scan();
    self.selection.pending = self.selection.remembered.get(&self.current_dir).cloned();
    self.entries = navigation_entries(&self.current_dir);
    self.max_visible_name_width = 0;
    self.timestamps = self
      .entries
      .iter()
      .map(|entry| {
        (
          entry.path.clone(),
          TimestampState::from_modified(modified_time(&entry.path)),
        )
      })
      .collect();
    self.visible_indices.clear();
    self.selected = 0;
    self.filter_query.clear();
    self.filter_kind = FilterKind::default();
    self.filter_mode = false;
    self.status = ScanStatus::Indexing;
    self.sort_entries();
    self.refresh_visible();
    self.select_current_directory();
    self.try_restore_pending_selection();

    if !bypass_cache
      && !self.show_files
      && let Some(cache) = self.cache.as_ref()
      && let Ok(Some(entries)) = cache.load(&self.current_dir)
    {
      self.entries.extend(entries);
      self.sort_entries();
      self.refresh_visible();
      self.finish_pending_selection();
      self.start_metadata_refresh();
      return;
    }

    self.scan_fingerprint = if self.show_files {
      None
    } else {
      self
        .cache
        .as_ref()
        .and_then(|_| DirectoryCache::fingerprint(&self.current_dir).ok())
    };

    match ScanHandle::start(self.current_dir.clone(), self.show_files) {
      Ok(scan) => self.scan = Some(scan),
      Err(error) => self.status = ScanStatus::Error(format!("unable to start scanner: {error}")),
    }
  }

  fn stop_scan(&mut self) {
    self.scan_fingerprint = None;
    if let Some(scan) = self.scan.take() {
      scan.cancel();
    }
  }

  fn start_metadata_refresh(&mut self) {
    let paths = self
      .entries
      .iter()
      .map(|entry| entry.path.clone())
      .collect::<Vec<_>>();
    self.timestamps = paths
      .iter()
      .cloned()
      .map(|path| (path, TimestampState::Pending))
      .collect();
    self.status = ScanStatus::Refreshing;
    match ScanHandle::start_metadata(paths) {
      Ok(scan) => self.scan = Some(scan),
      Err(error) => {
        self.mark_pending_timestamps_unavailable();
        self.status = ScanStatus::Error(format!("unable to start metadata refresh: {error}"));
      }
    }
  }

  fn persist_scan(&self, cacheable: bool) {
    if !cacheable || self.show_files {
      return;
    }
    let (Some(cache), Some(before)) = (self.cache.as_ref(), self.scan_fingerprint.as_ref()) else {
      return;
    };
    let navigation_count = self.navigation_count();
    let _ = cache.store_if_unchanged(&self.current_dir, before, &self.entries[navigation_count..]);
  }

  fn toggle_files(&mut self) {
    self.remember_selection();
    self.show_files = !self.show_files;
    self.start_scan(false);
  }

  fn open_selected(&mut self) {
    let Some(entry) = self.selected_entry().cloned() else {
      return;
    };
    if !entry.is_directory || entry.path == self.current_dir {
      return;
    }
    let path = entry.path;
    let previous_dir = self.current_dir.clone();
    let returning_to_parent = previous_dir
      .parent()
      .is_some_and(|parent| parent == path.as_path());
    self.remember_selection();
    self.current_dir = path;
    if returning_to_parent {
      self
        .selection
        .remembered
        .insert(self.current_dir.clone(), previous_dir);
    }
    self.start_scan(false);
  }

  fn open_parent(&mut self) {
    let Some(parent) = self.current_dir.parent().map(Path::to_path_buf) else {
      return;
    };
    if parent == self.current_dir.as_path() {
      return;
    }
    let child = self.current_dir.clone();
    self.remember_selection();
    self.current_dir = parent;
    self
      .selection
      .remembered
      .insert(self.current_dir.clone(), child);
    self.start_scan(false);
  }

  fn rescan(&mut self) {
    self.remember_selection();
    self.start_scan(true);
  }

  fn move_selection(&mut self, delta: isize) {
    self.cancel_pending_selection();
    if self.visible_indices.is_empty() {
      return;
    }
    let last = self.visible_indices.len().saturating_sub(1) as isize;
    self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
  }

  fn selected_path(&self) -> Option<PathBuf> {
    self.selected_entry().map(|entry| entry.path.clone())
  }

  fn selected_result_path(&self) -> Option<PathBuf> {
    let entry = self.selected_entry()?;
    if entry.is_directory {
      Some(entry.path.clone())
    } else {
      Some(self.current_dir.clone())
    }
  }

  fn selected_entry(&self) -> Option<&DirectoryEntry> {
    self
      .visible_indices
      .get(self.selected)
      .and_then(|&index| self.entries.get(index))
  }

  fn remember_selection(&mut self) {
    let Some(path) = self.selected_path() else {
      return;
    };
    self
      .selection
      .remembered
      .insert(self.current_dir.clone(), path);
  }

  fn set_filter_query(&mut self, query: String) {
    let selected_path = self.selected_path();
    self.filter_query = query;
    self.refresh_visible();
    self.restore_selection(selected_path.as_deref());
  }

  fn refresh_visible(&mut self) {
    self.visible_indices = match self.filter_kind {
      FilterKind::Substring => matching_indices(&self.entries, &self.filter_query),
      FilterKind::Fuzzy => fuzzy_indices(&self.entries, &self.filter_query),
    };
    self.max_visible_name_width = self
      .visible_indices
      .iter()
      .filter_map(|&index| self.entries.get(index))
      .map(|entry| UnicodeWidthStr::width(entry.name.as_str()))
      .max()
      .unwrap_or(0);
    self.selected = self
      .selected
      .min(self.visible_indices.len().saturating_sub(1));
  }

  fn sort_entries(&mut self) {
    let navigation_count = self.navigation_count();
    self.entries[navigation_count..].sort_unstable_by(|left, right| {
      right.is_directory.cmp(&left.is_directory).then_with(|| {
        left
          .name
          .cmp(&right.name)
          .then_with(|| left.path.cmp(&right.path))
      })
    });
  }

  fn files_position(&self) -> Option<usize> {
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

  fn selected_row(&self, files_position: Option<usize>) -> usize {
    self.selected + usize::from(files_position.is_some_and(|position| position <= self.selected))
  }

  fn entry_color(entry: &DirectoryEntry) -> Color {
    if entry.is_directory {
      Color::White
    } else {
      Color::DarkGrey
    }
  }

  fn restore_selection(&mut self, previous_path: Option<&Path>) {
    if let Some(previous_path) = previous_path
      && let Some(index) = self
        .visible_indices
        .iter()
        .position(|&index| self.entries[index].path == previous_path)
    {
      self.selected = index;
      return;
    }
    self.selected = self
      .selected
      .min(self.visible_indices.len().saturating_sub(1));
  }

  fn cancel_pending_selection(&mut self) {
    self.selection.pending = None;
  }

  fn current_directory_position(&self) -> Option<usize> {
    self
      .visible_indices
      .iter()
      .position(|&index| self.entries[index].path == self.current_dir)
  }

  fn select_current_directory(&mut self) {
    if let Some(position) = self.current_directory_position() {
      self.selected = position;
    } else {
      self.selected = self
        .selected
        .min(self.visible_indices.len().saturating_sub(1));
    }
  }

  fn try_restore_pending_selection(&mut self) -> bool {
    let Some(pending) = self.selection.pending.clone() else {
      return false;
    };
    let position = self
      .visible_indices
      .iter()
      .position(|&index| self.entries[index].path.as_path() == pending.as_path());
    let Some(position) = position else {
      return false;
    };
    self.selected = position;
    self.selection.pending = None;
    true
  }

  fn finish_pending_selection(&mut self) {
    let had_pending = self.selection.pending.is_some();
    if !self.try_restore_pending_selection() && had_pending {
      self.select_current_directory();
    }
    self.selection.pending = None;
    self.selected = self
      .selected
      .min(self.visible_indices.len().saturating_sub(1));
  }

  fn draw(&self, output: &mut Stdout) -> io::Result<()> {
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

  fn parent_count(&self) -> usize {
    let Some(parent) = self.current_dir.parent() else {
      return 0;
    };
    if parent == self.current_dir {
      return 0;
    }
    usize::from(
      self
        .entries
        .first()
        .is_some_and(|entry| entry.path == parent),
    )
  }

  fn navigation_count(&self) -> usize {
    let parent_count = self.parent_count();
    parent_count
      + usize::from(
        self
          .entries
          .get(parent_count)
          .is_some_and(|entry| entry.name == "." && entry.path == self.current_dir),
      )
  }

  fn discovered_count(&self) -> usize {
    self.entries.len().saturating_sub(self.navigation_count())
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

  fn scroll_start(&self, list_height: usize, selected_row: usize) -> usize {
    if list_height == 0 {
      return 0;
    }
    selected_row.saturating_sub(list_height.saturating_sub(1))
  }

  fn name_column_width(&self, width: usize) -> Option<usize> {
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

fn parent_entry(path: &Path) -> Option<DirectoryEntry> {
  let parent = path.parent()?;
  if parent == path {
    return None;
  }
  Some(DirectoryEntry {
    name: "..".to_owned(),
    path: parent.to_path_buf(),
    is_directory: true,
  })
}

fn navigation_entries(path: &Path) -> Vec<DirectoryEntry> {
  let mut entries = Vec::with_capacity(2);
  if let Some(parent) = parent_entry(path) {
    entries.push(parent);
  }
  entries.push(DirectoryEntry {
    name: ".".to_owned(),
    path: path.to_path_buf(),
    is_directory: true,
  });
  entries
}

fn format_timestamp(modified: SystemTime) -> Option<String> {
  let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
  let timestamp = OffsetDateTime::from_unix_timestamp_nanos(elapsed.as_nanos() as i128).ok()?;
  let offset = UtcOffset::local_offset_at(timestamp).ok()?;
  let formatted = timestamp.to_offset(offset).format(TIMESTAMP_FORMAT).ok()?;
  (UnicodeWidthStr::width(formatted.as_str()) == TIMESTAMP_WIDTH).then_some(formatted)
}

fn format_entry_line(
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

fn copy_path<W: Write>(output: &mut W, path: &Path) -> io::Result<()> {
  let Some(path) = path.to_str() else {
    return Ok(());
  };
  execute!(output, CopyToClipboard::to_clipboard_from(path))
}

#[cfg(test)]
mod tests {
  use super::*;

  const TEST_NARROW_LINE_WIDTH: usize = 32;
  const TEST_WIDE_LINE_WIDTH: usize = 80;
  const TEST_NARROW_NAME_COLUMN_WIDTH: usize = 10;
  const TEST_WIDE_NAME_COLUMN_WIDTH: usize = 24;
  const TEST_TINY_LINE_WIDTH: usize = 20;

  fn app_with_entries(current_dir: PathBuf, entries: Vec<DirectoryEntry>, selected: usize) -> App {
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

  fn finish_scan(app: &mut App) {
    while app.scan.is_some() {
      app.poll_scan_events();
      std::thread::yield_now();
    }
  }

  fn temporary_cache_root(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    std::env::temp_dir().join(format!("fast-app-{label}-{}-{suffix}", std::process::id()))
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

    assert!(action.is_none());
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

    assert!(
      app
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .is_none()
    );
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
}
