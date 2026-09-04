use std::{
  collections::HashMap,
  io::{self, Stdout, Write},
  path::{Path, PathBuf},
  time::Duration,
};

use crossterm::{
  cursor::MoveTo,
  event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
  queue,
  style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
  terminal::{self, Clear, ClearType},
};

use crate::{
  cache::{DirectoryCache, DirectoryFingerprint},
  filter::{FilterKind, fuzzy_indices, matching_indices},
  scan::{DirectoryEntry, ScanEvent, ScanHandle},
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
  selection: SelectionState,
  status: ScanStatus,
}

enum ScanStatus {
  Indexing,
  Ready,
  Error(String),
}

#[derive(Default)]
struct SelectionState {
  remembered: HashMap<PathBuf, PathBuf>,
  pending: Option<PendingSelection>,
}

#[derive(Clone)]
enum PendingSelection {
  Remembered(PathBuf),
  FirstChild,
}

pub(crate) enum ExitAction {
  Select(PathBuf),
  Cancel,
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
      selection: SelectionState::default(),
      status: ScanStatus::Indexing,
    };
    app.start_scan();
    app
  }

  pub(crate) fn run(&mut self, output: &mut Stdout) -> io::Result<ExitAction> {
    let action = loop {
      self.poll_scan_events();
      self.draw(output)?;

      if event::poll(EVENT_POLL_INTERVAL)?
        && let Event::Key(key) = event::read()?
        && let Some(action) = self.handle_key(key)
      {
        break action;
      }
    };

    self.stop_scan();
    Ok(action)
  }

  fn handle_key(&mut self, key: KeyEvent) -> Option<ExitAction> {
    if key.kind != KeyEventKind::Press {
      return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
      return Some(ExitAction::Cancel);
    }

    if self.filter_mode {
      return self.handle_filter_key(key);
    }

    match key.code {
      KeyCode::Esc => {
        if self.filter_query.is_empty() {
          Some(ExitAction::Cancel)
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
      KeyCode::Char('q') | KeyCode::Char('Q') => {
        self.selected_result_path().map(ExitAction::Select)
      }
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

  fn handle_filter_key(&mut self, key: KeyEvent) -> Option<ExitAction> {
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
      self.status = ScanStatus::Error("directory scanner stopped unexpectedly".to_owned());
    }
  }

  fn apply_scan_event(&mut self, event: ScanEvent) {
    match event {
      ScanEvent::Chunk(entries) => {
        let selected_path = self.selected_path();
        self.entries.extend(entries);
        self.sort_entries();
        self.refresh_visible();
        if !self.try_restore_pending_selection() {
          self.restore_selection(selected_path.as_deref());
        }
      }
      ScanEvent::Finished => {
        self.finish_pending_selection();
        self.persist_scan();
        self.status = ScanStatus::Ready;
        self.scan = None;
        self.scan_fingerprint = None;
      }
      ScanEvent::Error(error) => {
        self.selection.pending = None;
        self.status = ScanStatus::Error(error);
        self.scan = None;
        self.scan_fingerprint = None;
      }
    }
  }

  fn start_scan(&mut self) {
    self.stop_scan();
    self.selection.pending = Some(
      self
        .selection
        .remembered
        .get(&self.current_dir)
        .cloned()
        .map(PendingSelection::Remembered)
        .unwrap_or(PendingSelection::FirstChild),
    );
    self.entries = navigation_entries(&self.current_dir);
    self.visible_indices.clear();
    self.selected = 0;
    self.filter_query.clear();
    self.filter_kind = FilterKind::default();
    self.filter_mode = false;
    self.status = ScanStatus::Indexing;
    self.sort_entries();
    self.refresh_visible();
    self.select_first_child_or_navigation();
    self.try_restore_pending_selection();

    if !self.show_files
      && let Some(cache) = self.cache.as_ref()
      && let Ok(Some(entries)) = cache.load(&self.current_dir)
    {
      self.entries.extend(entries);
      self.sort_entries();
      self.refresh_visible();
      self.finish_pending_selection();
      self.status = ScanStatus::Ready;
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

  fn persist_scan(&self) {
    if self.show_files {
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
    self.start_scan();
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
    self.start_scan();
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
    self.start_scan();
  }

  fn rescan(&mut self) {
    self.remember_selection();
    self.start_scan();
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

  fn first_child_position(&self) -> Option<usize> {
    let navigation_count = self.navigation_count();
    self
      .visible_indices
      .iter()
      .position(|&index| index >= navigation_count && self.entries[index].is_directory)
  }

  fn navigation_position(&self) -> Option<usize> {
    let navigation_count = self.navigation_count();
    self
      .visible_indices
      .iter()
      .rposition(|&index| index < navigation_count)
  }

  fn select_first_child_or_navigation(&mut self) {
    if let Some(position) = self.first_child_position() {
      self.selected = position;
    } else if let Some(position) = self.navigation_position() {
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
    let position = match pending {
      PendingSelection::Remembered(path) => self
        .visible_indices
        .iter()
        .position(|&index| self.entries[index].path == path),
      PendingSelection::FirstChild => self.first_child_position(),
    };
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
      self.select_first_child_or_navigation();
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
      put_line(
        output,
        row as u16 + 2,
        &format!("{marker}{}", entry.name),
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
        " / filter  Up/Down or j/k  Enter/l open  F files {}  Backspace/h parent  r rescan  q select  Esc cancel",
        if self.show_files { "on" } else { "off" }
      )
    } else {
      format!(
        " / edit filter  Tab toggle mode  Up/Down or j/k  Enter/l open  F files {}  Backspace/h parent  r rescan  q select  Esc clear",
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

fn put_line<W: Write>(
  output: &mut W,
  row: u16,
  text: &str,
  width: usize,
  color: Color,
  selected: bool,
) -> io::Result<()> {
  let text = text.chars().take(width).collect::<String>();
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

#[cfg(test)]
mod tests {
  use super::*;

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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    }
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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    };

    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    match action {
      Some(ExitAction::Select(path)) => assert_eq!(path, selected_path),
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

    assert!(matches!(action, Some(ExitAction::Select(path)) if path == current_dir));
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
  fn mixed_listings_default_to_the_first_directory() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.show_files = true;
    app.selection.pending = Some(PendingSelection::FirstChild);

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

    assert_eq!(app.selected_path(), Some(current_dir.join("z-directory")));
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
  fn file_only_listings_fall_back_to_the_current_directory() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.show_files = true;
    app.selection.pending = Some(PendingSelection::FirstChild);

    app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
      name: "file.txt".to_owned(),
      path: current_dir.join("file.txt"),
      is_directory: false,
    }]));
    app.apply_scan_event(ScanEvent::Finished);

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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    };

    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

    assert!(matches!(action, Some(ExitAction::Select(path)) if path == current_dir));
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
      Some(ExitAction::Cancel)
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
  fn unvisited_directory_selects_the_first_child_after_navigation_entries() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);

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

    assert_eq!(app.selected_path(), Some(current_dir.join("alpha")));
    assert!(app.selection.pending.is_none());
  }

  #[test]
  fn root_directory_selects_the_first_child_after_current_entry() {
    let current_dir = PathBuf::from("/");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);

    app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
      name: "alpha".to_owned(),
      path: current_dir.join("alpha"),
      is_directory: true,
    }]));

    assert_eq!(app.selected_path(), Some(current_dir.join("alpha")));
  }

  #[test]
  fn empty_directory_falls_back_to_the_current_entry() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);
    app.apply_scan_event(ScanEvent::Finished);
    assert_eq!(app.selected_path(), Some(current_dir.clone()));

    let root = PathBuf::from("/");
    let mut app = app_with_entries(root.clone(), navigation_entries(&root), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);
    app.apply_scan_event(ScanEvent::Finished);
    assert_eq!(app.selected_path(), Some(root));
  }

  #[test]
  fn manual_selection_movement_overrides_pending_default() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);

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
  fn manual_home_and_end_override_pending_default() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 1);
    app.selection.pending = Some(PendingSelection::FirstChild);
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
      name: "alpha".to_owned(),
      path: current_dir.join("alpha"),
      is_directory: true,
    }]));
    assert_eq!(app.selected_path(), Some(PathBuf::from("/tmp")));

    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app.selection.pending = Some(PendingSelection::FirstChild);
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
    app.start_scan();
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
  fn missing_remembered_selection_falls_back_to_a_valid_entry() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = app_with_entries(current_dir.clone(), navigation_entries(&current_dir), 0);
    app
      .selection
      .remembered
      .insert(current_dir.clone(), current_dir.join("missing"));
    app.start_scan();
    app.stop_scan();

    app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
      name: "other".to_owned(),
      path: current_dir.join("other"),
      is_directory: true,
    }]));
    app.apply_scan_event(ScanEvent::Finished);

    assert_eq!(app.selected_path(), Some(current_dir.join("other")));
    assert!(app.selected < app.visible_indices.len());
  }

  #[test]
  fn uses_a_valid_cache_before_starting_a_scan() {
    let root = std::env::temp_dir().join(format!(
      "fast-app-cache-test-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
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
    assert!(matches!(app.status, ScanStatus::Ready));
    assert!(app.scan.is_none());
    assert_eq!(app.entries[0].name, "..");
    assert_eq!(app.entries[1].name, ".");
    assert_eq!(app.selected_path(), Some(child.clone()));
    assert_eq!(app.discovered_count(), 1);
    assert_eq!(
      app
        .entries
        .iter()
        .filter(|entry| entry.name == "child")
        .count(),
      1
    );

    app.selected = app
      .visible_indices
      .iter()
      .position(|&index| app.entries[index].path == child)
      .unwrap();
    app.remember_selection();
    app.start_scan();

    assert_eq!(app.selected_path(), Some(child.clone()));

    app.filter_kind = FilterKind::Substring;
    app.filter_query = "child".to_owned();
    app.start_scan();

    assert_eq!(app.filter_kind, FilterKind::Fuzzy);
    assert!(app.filter_query.is_empty());
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn does_not_persist_navigation_entries_in_the_cache() {
    let root = std::env::temp_dir().join(format!(
      "fast-app-navigation-cache-test-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    };
    app.entries.push(DirectoryEntry {
      name: "child".to_owned(),
      path: child.clone(),
      is_directory: true,
    });

    app.persist_scan();

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
    let root = std::env::temp_dir().join(format!(
      "fast-app-file-cache-test-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    };
    app.entries.push(DirectoryEntry {
      name: "file.txt".to_owned(),
      path: file,
      is_directory: false,
    });

    app.persist_scan();

    assert_eq!(cache.load(&directory).unwrap(), None);
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn file_visible_scan_ignores_directory_cache() {
    let root = std::env::temp_dir().join(format!(
      "fast-app-file-cache-read-test-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
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
      selection: SelectionState::default(),
      status: ScanStatus::Ready,
    };

    app.start_scan();

    assert!(app.scan.is_some());
    app.stop_scan();
    let _ = std::fs::remove_dir_all(root);
  }
}
