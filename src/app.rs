mod render;

use std::{
  collections::HashMap,
  io::{self, Stdout, Write},
  path::{Path, PathBuf},
  time::{Duration, SystemTime},
};

use crossterm::{
  clipboard::CopyToClipboard,
  event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
  execute,
};
use unicode_width::UnicodeWidthStr;

use crate::{
  cache::{DirectoryCache, DirectoryFingerprint},
  filter::{FilterKind, fuzzy_indices, matching_indices},
  scan::{DirectoryEntry, EntryMetadata, ScanEvent, ScanHandle, modified_time},
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

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
  Error(String),
  Indexing,
  Ready,
  Refreshing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TimestampState {
  Available(SystemTime),
  Pending,
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
  Cancel,
  Select(PathBuf),
}

enum KeyAction {
  Copy(PathBuf),
  Exit(ExitAction),
  Redraw,
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
    self.run_with(output, Self::draw, |scan_active| {
      // The scan channel cannot wake a terminal read, so poll only while it is active.
      if scan_active && !event::poll(EVENT_POLL_INTERVAL)? {
        Ok(None)
      } else {
        event::read().map(Some)
      }
    })
  }

  fn run_with<W: Write>(
    &mut self,
    output: &mut W,
    mut draw: impl FnMut(&Self, &mut W) -> io::Result<()>,
    mut next_event: impl FnMut(bool) -> io::Result<Option<Event>>,
  ) -> io::Result<ExitAction> {
    let mut redraw = true;
    let action = loop {
      redraw |= self.poll_scan_events();
      if redraw {
        draw(self, output)?;
      }

      redraw = match next_event(self.scan.is_some())?.and_then(|event| self.handle_event(event)) {
        Some(KeyAction::Exit(action)) => break action,
        Some(KeyAction::Copy(path)) => {
          copy_path(output, &path)?;
          false
        }
        Some(KeyAction::Redraw) => true,
        None => false,
      };
    };

    self.stop_scan();
    Ok(action)
  }

  fn handle_event(&mut self, event: Event) -> Option<KeyAction> {
    match event {
      Event::Key(key) => self.handle_key(key),
      Event::Resize(_, _) => Some(KeyAction::Redraw),
      _ => None,
    }
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
          Some(KeyAction::Redraw)
        }
      }
      KeyCode::Char('/') => {
        self.filter_mode = true;
        Some(KeyAction::Redraw)
      }
      KeyCode::Char('F') => {
        self.toggle_files();
        Some(KeyAction::Redraw)
      }
      KeyCode::Char('q') | KeyCode::Char('Q') => self
        .selected_result_path()
        .map(|path| KeyAction::Exit(ExitAction::Select(path))),
      KeyCode::Char('y') => self.selected_path().map(KeyAction::Copy),
      KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1).then_some(KeyAction::Redraw),
      KeyCode::Down | KeyCode::Char('j') => self.move_selection(1).then_some(KeyAction::Redraw),
      KeyCode::Home | KeyCode::Char('g') => self.select_index(0).then_some(KeyAction::Redraw),
      KeyCode::End | KeyCode::Char('G') => self
        .select_index(self.visible_indices.len().saturating_sub(1))
        .then_some(KeyAction::Redraw),
      KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
        self.open_selected().then_some(KeyAction::Redraw)
      }
      KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
        self.open_parent().then_some(KeyAction::Redraw)
      }
      KeyCode::Char('r') | KeyCode::Char('R') => {
        self.rescan();
        Some(KeyAction::Redraw)
      }
      _ => None,
    }
  }

  fn handle_filter_key(&mut self, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
      KeyCode::Esc => {
        self.filter_mode = false;
        self.set_filter_query(String::new());
        Some(KeyAction::Redraw)
      }
      KeyCode::Enter => {
        self.filter_mode = false;
        Some(KeyAction::Redraw)
      }
      KeyCode::Tab => {
        let selected_path = self.selected_path();
        self.filter_kind = self.filter_kind.toggle();
        self.refresh_visible();
        self.restore_selection(selected_path.as_deref());
        Some(KeyAction::Redraw)
      }
      KeyCode::Backspace => {
        let mut query = self.filter_query.clone();
        let changed = query.pop().is_some();
        self.set_filter_query(query);
        changed.then_some(KeyAction::Redraw)
      }
      KeyCode::Up => self.move_selection(-1).then_some(KeyAction::Redraw),
      KeyCode::Down => self.move_selection(1).then_some(KeyAction::Redraw),
      KeyCode::Home => self.select_index(0).then_some(KeyAction::Redraw),
      KeyCode::End => self
        .select_index(self.visible_indices.len().saturating_sub(1))
        .then_some(KeyAction::Redraw),
      KeyCode::Char(character)
        if !key
          .modifiers
          .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
      {
        let mut query = self.filter_query.clone();
        query.push(character);
        self.set_filter_query(query);
        Some(KeyAction::Redraw)
      }
      _ => None,
    }
  }

  fn poll_scan_events(&mut self) -> bool {
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

    let changed = !events.is_empty() || (disconnected && self.scan.is_some());
    for event in events {
      self.apply_scan_event(event);
    }
    if disconnected && self.scan.is_some() {
      self.scan = None;
      self.mark_pending_timestamps_unavailable();
      self.status = ScanStatus::Error("directory scanner stopped unexpectedly".to_owned());
    }
    changed
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

  fn open_selected(&mut self) -> bool {
    let Some(entry) = self.selected_entry().cloned() else {
      return false;
    };
    if !entry.is_directory || entry.path == self.current_dir {
      return false;
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
    true
  }

  fn open_parent(&mut self) -> bool {
    let Some(parent) = self.current_dir.parent().map(Path::to_path_buf) else {
      return false;
    };
    if parent == self.current_dir.as_path() {
      return false;
    }
    let child = self.current_dir.clone();
    self.remember_selection();
    self.current_dir = parent;
    self
      .selection
      .remembered
      .insert(self.current_dir.clone(), child);
    self.start_scan(false);
    true
  }

  fn rescan(&mut self) {
    self.remember_selection();
    self.start_scan(true);
  }

  fn move_selection(&mut self, delta: isize) -> bool {
    self.cancel_pending_selection();
    if self.visible_indices.is_empty() {
      return false;
    }
    let previous = self.selected;
    let last = self.visible_indices.len().saturating_sub(1) as isize;
    self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    self.selected != previous
  }

  fn select_index(&mut self, index: usize) -> bool {
    self.cancel_pending_selection();
    let previous = self.selected;
    self.selected = index;
    self.selected != previous
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

fn copy_path<W: Write>(output: &mut W, path: &Path) -> io::Result<()> {
  let Some(path) = path.to_str() else {
    return Ok(());
  };
  execute!(output, CopyToClipboard::to_clipboard_from(path))
}

#[cfg(test)]
#[path = "../test/app.rs"]
mod tests;
