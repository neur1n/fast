mod scan;

use std::{
  env,
  ffi::OsString,
  fs,
  io::{self, Stdout, Write, stdout},
  path::{Path, PathBuf},
  time::Duration,
};

use crossterm::{
  cursor::{Hide, MoveTo, Show},
  event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
  execute, queue,
  style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
  terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
  },
};

use scan::{DirectoryEntry, ScanEvent, ScanHandle};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct App {
  current_dir: PathBuf,
  entries: Vec<DirectoryEntry>,
  selected: usize,
  scan: Option<ScanHandle>,
  status: ScanStatus,
}

enum ScanStatus {
  Indexing,
  Ready,
  Error(String),
}

enum ExitAction {
  Select(PathBuf),
  Cancel,
}

#[derive(Debug, Eq, PartialEq)]
enum CliError {
  Help,
  Invalid(String),
}

impl App {
  fn new(current_dir: PathBuf) -> Self {
    let mut app = Self {
      current_dir,
      entries: Vec::new(),
      selected: 0,
      scan: None,
      status: ScanStatus::Indexing,
    };
    app.start_scan();
    app
  }

  fn run(&mut self, output: &mut Stdout) -> io::Result<ExitAction> {
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

    match key.code {
      KeyCode::Esc => Some(ExitAction::Cancel),
      KeyCode::Char('q') | KeyCode::Char('Q') => Some(ExitAction::Select(self.selected_path())),
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
        self.selected = self.entries.len().saturating_sub(1);
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
        let selected_path = self
          .entries
          .get(self.selected)
          .map(|entry| entry.path.clone());
        self.entries.extend(entries);
        self.sort_entries();
        self.restore_selection(selected_path.as_deref());
      }
      ScanEvent::Finished => {
        self.status = ScanStatus::Ready;
        self.scan = None;
      }
      ScanEvent::Error(error) => {
        self.status = ScanStatus::Error(error);
        self.scan = None;
      }
    }
  }

  fn start_scan(&mut self) {
    self.stop_scan();
    self.entries = parent_entry(&self.current_dir).into_iter().collect();
    self.selected = 0;
    self.status = ScanStatus::Indexing;
    self.sort_entries();

    match ScanHandle::start(self.current_dir.clone()) {
      Ok(scan) => self.scan = Some(scan),
      Err(error) => self.status = ScanStatus::Error(format!("unable to start scanner: {error}")),
    }
  }

  fn stop_scan(&mut self) {
    if let Some(scan) = self.scan.take() {
      scan.cancel();
    }
  }

  fn open_selected(&mut self) {
    let Some(entry) = self.entries.get(self.selected) else {
      return;
    };
    self.current_dir = entry.path.clone();
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
    if self.entries.is_empty() {
      return;
    }
    let last = self.entries.len().saturating_sub(1) as isize;
    self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
  }

  fn selected_path(&self) -> PathBuf {
    self
      .entries
      .get(self.selected)
      .map(|entry| entry.path.clone())
      .unwrap_or_else(|| self.current_dir.clone())
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
        .entries
        .iter()
        .position(|entry| entry.path == previous_path)
    {
      self.selected = index;
      return;
    }
    self.selected = self.selected.min(self.entries.len().saturating_sub(1));
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
      let Some(entry) = self.entries.get(index) else {
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
      " Up/Down or j/k  Enter/l open  Backspace/h parent  r rescan  q select  Esc cancel",
      width,
      Color::DarkGrey,
      false,
    )?;
    output.flush()
  }

  fn status_text(&self) -> String {
    match &self.status {
      ScanStatus::Indexing => {
        format!(
          " Indexing... {} directories discovered",
          self.discovered_count()
        )
      }
      ScanStatus::Ready => format!(" Ready  {} directories", self.discovered_count()),
      ScanStatus::Error(error) => format!(" Error  {error}"),
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

fn parse_args<I>(args: I) -> Result<Option<PathBuf>, CliError>
where
  I: IntoIterator<Item = OsString>,
{
  let mut selection_file = None;
  let mut args = args.into_iter();
  while let Some(argument) = args.next() {
    let Some(argument) = argument.to_str() else {
      return Err(CliError::Invalid(
        "arguments must be valid UTF-8".to_owned(),
      ));
    };
    match argument {
      "-h" | "--help" => return Err(CliError::Help),
      "--select" => {
        if selection_file.is_some() {
          return Err(CliError::Invalid(
            "--select may only be provided once".to_owned(),
          ));
        }
        let Some(path) = args.next() else {
          return Err(CliError::Invalid("--select requires a path".to_owned()));
        };
        if path.is_empty() {
          return Err(CliError::Invalid(
            "--select requires a non-empty path".to_owned(),
          ));
        }
        selection_file = Some(PathBuf::from(path));
      }
      argument => {
        return Err(CliError::Invalid(format!("unknown argument: {argument}")));
      }
    }
  }
  Ok(selection_file)
}

fn selection_bytes(path: &Path) -> io::Result<Vec<u8>> {
  let path = path.to_str().ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      "selected path is not valid UTF-8",
    )
  })?;
  let mut bytes = Vec::with_capacity(path.len() + 1);
  bytes.extend_from_slice(path.as_bytes());
  bytes.push(0);
  Ok(bytes)
}

fn write_selection_file(selection_file: &Path, selected_path: &Path) -> io::Result<()> {
  fs::write(selection_file, selection_bytes(selected_path)?)
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

struct TerminalSession {
  output: Stdout,
}

impl TerminalSession {
  fn enter() -> io::Result<Self> {
    let mut output = stdout();
    enable_raw_mode()?;
    if let Err(error) = execute!(&mut output, EnterAlternateScreen, Hide) {
      let _ = disable_raw_mode();
      return Err(error);
    }
    Ok(Self { output })
  }
}

impl Drop for TerminalSession {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
    let _ = execute!(&mut self.output, Show, LeaveAlternateScreen, ResetColor);
  }
}

fn run() -> io::Result<i32> {
  let selection_file = match parse_args(env::args_os().skip(1)) {
    Ok(selection_file) => selection_file,
    Err(CliError::Help) => {
      print_help();
      return Ok(0);
    }
    Err(CliError::Invalid(message)) => {
      eprintln!("fast: {message}");
      eprintln!("Try `fast --help` for usage.");
      return Ok(2);
    }
  };
  let current_dir = env::current_dir()?;
  let mut terminal = TerminalSession::enter()?;
  let mut app = App::new(current_dir);
  let action = app.run(&mut terminal.output)?;
  drop(terminal);

  match action {
    ExitAction::Select(selected_path) => {
      if let Some(selection_file) = selection_file {
        write_selection_file(&selection_file, &selected_path)?;
      }
      Ok(0)
    }
    ExitAction::Cancel => Ok(if selection_file.is_some() { 1 } else { 0 }),
  }
}

fn print_help() {
  println!("Usage: fast [--select PATH]");
  println!();
  println!("Browse directories from the current working directory.");
  println!("  --select PATH  write the selected directory on confirmation");
  println!();
  println!("  q          select the highlighted directory");
  println!("  Esc/Ctrl-C  cancel without selecting a directory");
}

fn main() {
  let code = match run() {
    Ok(code) => code,
    Err(error) => {
      eprintln!("fast: {error}");
      1
    }
  };
  if code != 0 {
    std::process::exit(code);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_select_argument() {
    let args = [OsString::from("--select"), OsString::from("result")];
    assert_eq!(parse_args(args), Ok(Some(PathBuf::from("result"))));
  }

  #[test]
  fn selection_protocol_is_nul_terminated() {
    assert_eq!(
      selection_bytes(Path::new("/tmp/selected\npath")).unwrap(),
      b"/tmp/selected\npath\0"
    );
  }

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
      selected: 0,
      scan: None,
      status: ScanStatus::Ready,
    };

    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    match action {
      Some(ExitAction::Select(path)) => assert_eq!(path, selected_path),
      _ => panic!("q should select the highlighted entry"),
    }
  }
}
