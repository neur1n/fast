use std::{
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
  entries: Vec<DirectoryEntry>,
  visible_indices: Vec<usize>,
  selected: usize,
  filter_query: String,
  filter_kind: FilterKind,
  filter_mode: bool,
  cache: Option<DirectoryCache>,
  scan: Option<ScanHandle>,
  scan_fingerprint: Option<DirectoryFingerprint>,
  status: ScanStatus,
}

enum ScanStatus {
  Indexing,
  Ready,
  Error(String),
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
      entries: Vec::new(),
      visible_indices: Vec::new(),
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache,
      scan: None,
      scan_fingerprint: None,
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
      KeyCode::Char('q') | KeyCode::Char('Q') => self.selected_path().map(ExitAction::Select),
      KeyCode::Up | KeyCode::Char('k') => {
        self.move_selection(-1);
        None
      }
      KeyCode::Down | KeyCode::Char('j') => {
        self.move_selection(1);
        None
      }
      KeyCode::Home | KeyCode::Char('g') => {
        self.selected = 0;
        None
      }
      KeyCode::End | KeyCode::Char('G') => {
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
        self.start_scan();
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
        self.selected = 0;
        None
      }
      KeyCode::End => {
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
        self.restore_selection(selected_path.as_deref());
      }
      ScanEvent::Finished => {
        self.persist_scan();
        self.status = ScanStatus::Ready;
        self.scan = None;
        self.scan_fingerprint = None;
      }
      ScanEvent::Error(error) => {
        self.status = ScanStatus::Error(error);
        self.scan = None;
        self.scan_fingerprint = None;
      }
    }
  }

  fn start_scan(&mut self) {
    self.stop_scan();
    self.entries = parent_entry(&self.current_dir).into_iter().collect();
    self.visible_indices.clear();
    self.selected = 0;
    self.filter_query.clear();
    self.filter_kind = FilterKind::Substring;
    self.filter_mode = false;
    self.status = ScanStatus::Indexing;
    self.sort_entries();
    self.refresh_visible();

    if let Some(cache) = self.cache.as_ref()
      && let Ok(Some(entries)) = cache.load(&self.current_dir)
    {
      self.entries.extend(entries);
      self.sort_entries();
      self.refresh_visible();
      self.status = ScanStatus::Ready;
      return;
    }

    self.scan_fingerprint = self
      .cache
      .as_ref()
      .and_then(|_| DirectoryCache::fingerprint(&self.current_dir).ok());

    match ScanHandle::start(self.current_dir.clone()) {
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
    let (Some(cache), Some(before)) = (self.cache.as_ref(), self.scan_fingerprint.as_ref()) else {
      return;
    };
    let parent_count = self.parent_count();
    let _ = cache.store_if_unchanged(&self.current_dir, before, &self.entries[parent_count..]);
  }

  fn open_selected(&mut self) {
    let Some(path) = self.selected_path() else {
      return;
    };
    self.current_dir = path;
    self.start_scan();
  }

  fn open_parent(&mut self) {
    let Some(parent) = self.current_dir.parent().map(Path::to_path_buf) else {
      return;
    };
    if parent == self.current_dir {
      return;
    }
    self.current_dir = parent;
    self.start_scan();
  }

  fn move_selection(&mut self, delta: isize) {
    if self.visible_indices.is_empty() {
      return;
    }
    let last = self.visible_indices.len().saturating_sub(1) as isize;
    self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
  }

  fn selected_path(&self) -> Option<PathBuf> {
    self
      .visible_indices
      .get(self.selected)
      .and_then(|&index| self.entries.get(index))
      .map(|entry| entry.path.clone())
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
    let parent_count = self.parent_count();
    self.entries[parent_count..].sort_unstable_by(|left, right| {
      left
        .name
        .cmp(&right.name)
        .then_with(|| left.path.cmp(&right.path))
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
    let scroll_start = self.scroll_start(list_height);
    for row in 0..list_height {
      let index = scroll_start + row;
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
        Color::White,
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
          " Indexing... {} directories discovered",
          self.discovered_count()
        )
      }
      ScanStatus::Ready => format!(" Ready  {} directories", self.discovered_count()),
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
    usize::from(
      self
        .entries
        .first()
        .is_some_and(|entry| entry.path == self.current_dir.parent().unwrap_or(&self.current_dir)),
    )
  }

  fn discovered_count(&self) -> usize {
    self.entries.len().saturating_sub(self.parent_count())
  }

  fn footer_text(&self) -> String {
    if self.filter_mode {
      " Type to filter  Tab toggle mode  Backspace edit  Enter keep  Esc clear  Ctrl-C cancel"
        .to_owned()
    } else if self.filter_query.is_empty() {
      " / filter  Up/Down or j/k  Enter/l open  Backspace/h parent  r rescan  q select  Esc cancel"
        .to_owned()
    } else {
      " / edit filter  Tab toggle mode  Up/Down or j/k  Enter/l open  Backspace/h parent  r rescan  q select  Esc clear"
        .to_owned()
    }
  }

  fn scroll_start(&self, list_height: usize) -> usize {
    if list_height == 0 {
      return 0;
    }
    self.selected.saturating_sub(list_height.saturating_sub(1))
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
  })
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

  #[test]
  fn q_selects_the_highlighted_entry() {
    let current_dir = PathBuf::from("/tmp/current");
    let selected_path = current_dir.join("target");
    let mut app = App {
      current_dir,
      entries: vec![DirectoryEntry {
        name: "target".to_owned(),
        path: selected_path.clone(),
      }],
      visible_indices: vec![0],
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache: None,
      scan: None,
      scan_fingerprint: None,
      status: ScanStatus::Ready,
    };

    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    match action {
      Some(ExitAction::Select(path)) => assert_eq!(path, selected_path),
      _ => panic!("q should select the highlighted entry"),
    }
  }

  #[test]
  fn filter_input_updates_visible_entries() {
    let current_dir = PathBuf::from("/tmp/current");
    let mut app = App {
      current_dir: current_dir.clone(),
      entries: vec![
        DirectoryEntry {
          name: "..".to_owned(),
          path: PathBuf::from("/tmp"),
        },
        DirectoryEntry {
          name: "Target".to_owned(),
          path: current_dir.join("Target"),
        },
        DirectoryEntry {
          name: "logs".to_owned(),
          path: current_dir.join("logs"),
        },
      ],
      visible_indices: Vec::new(),
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache: None,
      scan: None,
      scan_fingerprint: None,
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
    assert_eq!(visible_names, vec!["..", "Target"]);
  }

  #[test]
  fn empty_results_cannot_select_the_current_directory() {
    let mut app = App {
      current_dir: PathBuf::from("/"),
      entries: vec![DirectoryEntry {
        name: "target".to_owned(),
        path: PathBuf::from("/target"),
      }],
      visible_indices: Vec::new(),
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache: None,
      scan: None,
      scan_fingerprint: None,
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
      entries: vec![
        DirectoryEntry {
          name: "target".to_owned(),
          path: current_dir.join("target"),
        },
        DirectoryEntry {
          name: "logs".to_owned(),
          path: selected_path.clone(),
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
      entries: vec![
        DirectoryEntry {
          name: "target".to_owned(),
          path: current_dir.join("target"),
        },
        DirectoryEntry {
          name: "logs".to_owned(),
          path: current_dir.join("logs"),
        },
      ],
      visible_indices: Vec::new(),
      selected: 0,
      filter_query: String::new(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache: None,
      scan: None,
      scan_fingerprint: None,
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
    assert_eq!(app.filter_kind, FilterKind::Fuzzy);
    assert_eq!(app.visible_indices, vec![0]);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.filter_kind, FilterKind::Substring);
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
      entries: vec![
        DirectoryEntry {
          name: "..".to_owned(),
          path: PathBuf::from("/tmp"),
        },
        DirectoryEntry {
          name: "alpha".to_owned(),
          path: selected_path.clone(),
        },
      ],
      visible_indices: Vec::new(),
      selected: 1,
      filter_query: "a".to_owned(),
      filter_kind: FilterKind::Substring,
      filter_mode: false,
      cache: None,
      scan: None,
      scan_fingerprint: None,
      status: ScanStatus::Indexing,
    };
    app.refresh_visible();

    app.apply_scan_event(ScanEvent::Chunk(vec![DirectoryEntry {
      name: "beta".to_owned(),
      path: current_dir.join("beta"),
    }]));

    assert_eq!(app.entries.len(), 3);
    assert_eq!(app.visible_indices, vec![0, 1, 2]);
    assert_eq!(app.selected_path(), Some(selected_path));
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
      path: child,
    }];
    let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
    assert!(
      cache
        .store_if_unchanged(&directory, &fingerprint, &entries)
        .unwrap()
    );

    let app = App::with_cache(directory, Some(cache));

    assert!(matches!(app.status, ScanStatus::Ready));
    assert!(app.scan.is_none());
    assert_eq!(
      app
        .entries
        .iter()
        .filter(|entry| entry.name == "child")
        .count(),
      1
    );
    let _ = std::fs::remove_dir_all(root);
  }
}
