use std::{
  fs, io,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
  },
  thread,
  time::{Duration, SystemTime},
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
  Error(String),
  Finished { cacheable: bool },
  MetadataChunk(Vec<EntryMetadata>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EntryMetadata {
  pub path: PathBuf,
  pub modified: Option<SystemTime>,
}

pub struct ScanHandle {
  receiver: Receiver<ScanEvent>,
  cancel: Arc<AtomicBool>,
}

impl ScanHandle {
  pub fn start(path: PathBuf, include_files: bool) -> io::Result<Self> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    thread::Builder::new()
      .name("fast-directory-scan".to_owned())
      .spawn(move || scan_directory(path, sender, worker_cancel, include_files))?;

    Ok(Self { receiver, cancel })
  }

  pub fn start_metadata(paths: Vec<PathBuf>) -> io::Result<Self> {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    thread::Builder::new()
      .name("fast-entry-metadata".to_owned())
      .spawn(move || refresh_metadata(paths, sender, worker_cancel))?;

    Ok(Self { receiver, cancel })
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
  let mut metadata_chunk = Vec::with_capacity(CHUNK_SIZE);
  let mut cacheable = !include_files;
  for result in directory {
    if is_cancelled(&cancel) {
      return;
    }

    let entry = match result {
      Ok(entry) => entry,
      Err(_) => {
        cacheable = false;
        continue;
      }
    };
    let path = entry.path();
    let is_directory = match resolve_directory(&entry, &path) {
      Ok(is_directory) => is_directory,
      Err(()) => {
        cacheable = false;
        false
      }
    };
    if !is_directory && !include_files {
      continue;
    }

    chunk.push(DirectoryEntry {
      name: entry.file_name().to_string_lossy().into_owned(),
      path: path.clone(),
      is_directory,
    });
    metadata_chunk.push(EntryMetadata {
      modified: modified_time(&path),
      path,
    });
    if chunk.len() == CHUNK_SIZE {
      if !send_event(&sender, ScanEvent::Chunk(chunk), &cancel) {
        return;
      }
      if !send_event(&sender, ScanEvent::MetadataChunk(metadata_chunk), &cancel) {
        return;
      }
      chunk = Vec::with_capacity(CHUNK_SIZE);
      metadata_chunk = Vec::with_capacity(CHUNK_SIZE);
    }
  }

  if !chunk.is_empty() {
    if !send_event(&sender, ScanEvent::Chunk(chunk), &cancel) {
      return;
    }
    if !send_event(&sender, ScanEvent::MetadataChunk(metadata_chunk), &cancel) {
      return;
    }
  }
  let _ = send_event(&sender, ScanEvent::Finished { cacheable }, &cancel);
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum EntryType {
  Directory,
  File,
  Symlink,
  Unknown,
}

fn resolve_directory(entry: &fs::DirEntry, path: &Path) -> Result<bool, ()> {
  let entry_type = entry
    .file_type()
    .map(classify_file_type)
    .unwrap_or(EntryType::Unknown);
  resolve_entry_type(entry_type, path)
}

fn resolve_entry_type(entry_type: EntryType, path: &Path) -> Result<bool, ()> {
  match entry_type {
    EntryType::Directory => Ok(true),
    EntryType::File => Ok(false),
    EntryType::Symlink | EntryType::Unknown => fs::metadata(path)
      .map(|metadata| metadata.is_dir())
      .map_err(|_| ()),
  }
}

fn classify_file_type(file_type: fs::FileType) -> EntryType {
  if file_type.is_dir() {
    EntryType::Directory
  } else if file_type.is_file() {
    EntryType::File
  } else if file_type.is_symlink() {
    EntryType::Symlink
  } else {
    EntryType::Unknown
  }
}

fn refresh_metadata(paths: Vec<PathBuf>, sender: SyncSender<ScanEvent>, cancel: Arc<AtomicBool>) {
  let mut chunk = Vec::with_capacity(CHUNK_SIZE);
  for path in paths {
    if is_cancelled(&cancel) {
      return;
    }
    chunk.push(EntryMetadata {
      modified: modified_time(&path),
      path,
    });
    if chunk.len() == CHUNK_SIZE {
      if !send_event(&sender, ScanEvent::MetadataChunk(chunk), &cancel) {
        return;
      }
      chunk = Vec::with_capacity(CHUNK_SIZE);
    }
  }

  if !chunk.is_empty() && !send_event(&sender, ScanEvent::MetadataChunk(chunk), &cancel) {
    return;
  }
  let _ = send_event(&sender, ScanEvent::Finished { cacheable: false }, &cancel);
}

pub(crate) fn modified_time(path: &std::path::Path) -> Option<SystemTime> {
  fs::symlink_metadata(path).ok()?.modified().ok()
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
#[path = "../test/scan.rs"]
mod tests;
