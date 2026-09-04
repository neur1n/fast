use std::{
  fs, io,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
  },
  thread::{self, JoinHandle},
  time::Duration,
};

pub const CHUNK_SIZE: usize = 64;

const CHANNEL_CAPACITY: usize = 4;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DirectoryEntry {
  pub name: String,
  pub path: PathBuf,
  pub is_directory: bool,
}

#[derive(Debug)]
pub enum ScanEvent {
  Chunk(Vec<DirectoryEntry>),
  Finished,
  Error(String),
}

pub struct ScanHandle {
  receiver: Receiver<ScanEvent>,
  cancel: Arc<AtomicBool>,
  worker: Option<JoinHandle<()>>,
}

impl ScanHandle {
  pub fn start(path: PathBuf, include_files: bool) -> io::Result<Self> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let worker = thread::Builder::new()
      .name("fast-directory-scan".to_owned())
      .spawn(move || scan_directory(path, sender, worker_cancel, include_files))?;

    Ok(Self {
      receiver,
      cancel,
      worker: Some(worker),
    })
  }

  pub fn try_recv(&self) -> Result<ScanEvent, TryRecvError> {
    self.receiver.try_recv()
  }

  pub fn cancel(&self) {
    self.cancel.store(true, Ordering::Release);
  }
}

impl Drop for ScanHandle {
  fn drop(&mut self) {
    self.cancel();
    // Dropping the join handle keeps navigation responsive when a filesystem
    // call is slow. The worker exits when its receiver is dropped or the flag
    // is observed.
    let _ = self.worker.take();
  }
}

fn scan_directory(
  path: PathBuf,
  sender: SyncSender<ScanEvent>,
  cancel: Arc<AtomicBool>,
  include_files: bool,
) {
  let directory = match fs::read_dir(&path) {
    Ok(directory) => directory,
    Err(error) => {
      let message = format!("unable to read {}: {error}", path.display());
      let _ = send_event(&sender, ScanEvent::Error(message), &cancel);
      return;
    }
  };

  let mut chunk = Vec::with_capacity(CHUNK_SIZE);
  for result in directory {
    if is_cancelled(&cancel) {
      return;
    }

    let entry = match result {
      Ok(entry) => entry,
      Err(_) => continue,
    };
    let file_type = match entry.file_type() {
      Ok(file_type) => file_type,
      Err(_) => continue,
    };
    let path = entry.path();
    let is_directory = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
    if !is_directory && !include_files {
      continue;
    }

    chunk.push(DirectoryEntry {
      name: entry.file_name().to_string_lossy().into_owned(),
      path,
      is_directory,
    });
    if chunk.len() == CHUNK_SIZE {
      if !send_event(&sender, ScanEvent::Chunk(chunk), &cancel) {
        return;
      }
      chunk = Vec::with_capacity(CHUNK_SIZE);
    }
  }

  if !chunk.is_empty() && !send_event(&sender, ScanEvent::Chunk(chunk), &cancel) {
    return;
  }
  let _ = send_event(&sender, ScanEvent::Finished, &cancel);
}

fn is_cancelled(cancel: &AtomicBool) -> bool {
  cancel.load(Ordering::Acquire)
}

fn send_event(sender: &SyncSender<ScanEvent>, mut event: ScanEvent, cancel: &AtomicBool) -> bool {
  loop {
    if is_cancelled(cancel) {
      return false;
    }

    match sender.try_send(event) {
      Ok(()) => return true,
      Err(TrySendError::Full(returned)) => {
        event = returned;
        thread::sleep(Duration::from_millis(2));
      }
      Err(TrySendError::Disconnected(_)) => return false,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    env,
    fs::{self, File},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
  };

  static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

  struct TemporaryDirectory(PathBuf);

  impl TemporaryDirectory {
    fn new() -> Self {
      let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
      let id = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
      let path = env::temp_dir().join(format!(
        "fast-scan-test-{}-{suffix}-{id}",
        std::process::id()
      ));
      fs::create_dir(&path).expect("temporary directory should be created");
      Self(path)
    }
  }

  impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  #[test]
  fn emits_only_directories() {
    let root = TemporaryDirectory::new();
    fs::create_dir(root.0.join("z-directory")).expect("directory should be created");
    fs::create_dir(root.0.join("a-directory")).expect("directory should be created");
    File::create(root.0.join("file.txt")).expect("file should be created");

    let (sender, receiver) = mpsc::sync_channel(2);
    let cancel = Arc::new(AtomicBool::new(false));
    scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

    let mut entries = Vec::new();
    let mut finished = false;
    while let Ok(event) = receiver.try_recv() {
      match event {
        ScanEvent::Chunk(chunk) => entries.extend(chunk),
        ScanEvent::Finished => finished = true,
        ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
      }
    }

    let mut names = entries
      .iter()
      .map(|entry| entry.name.as_str())
      .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["a-directory", "z-directory"]);
    assert!(entries.iter().all(|entry| entry.is_directory));
    assert!(finished);
  }

  #[test]
  fn reports_missing_directory() {
    let missing = env::temp_dir().join(format!(
      "fast-scan-missing-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos()
    ));
    let (sender, receiver) = mpsc::sync_channel(2);
    let cancel = Arc::new(AtomicBool::new(false));
    scan_directory(missing, sender, Arc::clone(&cancel), false);

    match receiver.try_recv().expect("scan error should be sent") {
      ScanEvent::Error(message) => assert!(message.contains("unable to read")),
      event => panic!("unexpected event: {event:?}"),
    }
  }

  #[test]
  fn splits_large_directories_into_chunks() {
    let root = TemporaryDirectory::new();
    for index in 0..(CHUNK_SIZE + 1) {
      fs::create_dir(root.0.join(format!("directory-{index:03}")))
        .expect("directory should be created");
    }

    let (sender, receiver) = mpsc::sync_channel(4);
    let cancel = Arc::new(AtomicBool::new(false));
    scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

    let mut chunk_count = 0;
    let mut entry_count = 0;
    let mut finished = false;
    while let Ok(event) = receiver.try_recv() {
      match event {
        ScanEvent::Chunk(entries) => {
          chunk_count += 1;
          entry_count += entries.len();
        }
        ScanEvent::Finished => finished = true,
        ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
      }
    }

    assert_eq!(entry_count, CHUNK_SIZE + 1);
    assert_eq!(chunk_count, 2);
    assert!(finished);
  }

  #[test]
  fn cancellation_prevents_events() {
    let root = TemporaryDirectory::new();
    let (sender, receiver) = mpsc::sync_channel(2);
    let cancel = Arc::new(AtomicBool::new(true));
    scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

    assert!(receiver.try_recv().is_err());
  }

  #[test]
  fn includes_files_when_requested() {
    let root = TemporaryDirectory::new();
    fs::create_dir(root.0.join("directory")).expect("directory should be created");
    File::create(root.0.join("file.txt")).expect("file should be created");

    let (sender, receiver) = mpsc::sync_channel(2);
    let cancel = Arc::new(AtomicBool::new(false));
    scan_directory(root.0.clone(), sender, Arc::clone(&cancel), true);

    let mut entries = Vec::new();
    while let Ok(event) = receiver.try_recv() {
      match event {
        ScanEvent::Chunk(chunk) => entries.extend(chunk),
        ScanEvent::Finished => {}
        ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
      }
    }

    entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "directory");
    assert!(entries[0].is_directory);
    assert_eq!(entries[1].name, "file.txt");
    assert!(!entries[1].is_directory);
  }

  #[test]
  fn file_entries_are_emitted_in_chunks() {
    let root = TemporaryDirectory::new();
    for index in 0..(CHUNK_SIZE + 1) {
      File::create(root.0.join(format!("file-{index:03}.txt"))).expect("file should be created");
    }

    let (sender, receiver) = mpsc::sync_channel(4);
    let cancel = Arc::new(AtomicBool::new(false));
    scan_directory(root.0.clone(), sender, Arc::clone(&cancel), true);

    let mut chunk_count = 0;
    let mut entry_count = 0;
    let mut finished = false;
    while let Ok(event) = receiver.try_recv() {
      match event {
        ScanEvent::Chunk(entries) => {
          chunk_count += 1;
          entry_count += entries.len();
          assert!(entries.iter().all(|entry| !entry.is_directory));
        }
        ScanEvent::Finished => finished = true,
        ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
      }
    }

    assert_eq!(entry_count, CHUNK_SIZE + 1);
    assert_eq!(chunk_count, 2);
    assert!(finished);
  }
}
