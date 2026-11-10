use std::{
  env,
  fs::{self, File},
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};

use super::*;

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

  let (sender, receiver) = mpsc::sync_channel(4);
  let cancel = Arc::new(AtomicBool::new(false));
  scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

  let mut entries = Vec::new();
  let mut finished = false;
  let mut cacheable = false;
  while let Ok(event) = receiver.try_recv() {
    match event {
      ScanEvent::Chunk(chunk) => entries.extend(chunk),
      ScanEvent::MetadataChunk(_) => {}
      ScanEvent::Finished {
        cacheable: finished_cacheable,
      } => {
        finished = true;
        cacheable = finished_cacheable;
      }
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
  assert!(cacheable);
}

#[test]
fn metadata_fallback_resolves_unknown_directory_types() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("directory");
  fs::create_dir(&directory).expect("directory should be created");
  let file = root.0.join("file.txt");
  File::create(&file).expect("file should be created");

  assert_eq!(resolve_entry_type(EntryType::Unknown, &directory), Ok(true));
  assert_eq!(resolve_entry_type(EntryType::Unknown, &file), Ok(false));
}

#[test]
fn metadata_fallback_failure_makes_type_unresolved() {
  let missing = env::temp_dir().join(format!(
    "fast-scan-type-missing-{}-{}",
    std::process::id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock should be after the Unix epoch")
      .as_nanos()
  ));

  assert!(resolve_entry_type(EntryType::Unknown, &missing).is_err());
}

#[test]
fn directory_without_child_directories_is_cacheable() {
  let root = TemporaryDirectory::new();
  File::create(root.0.join("file.txt")).expect("file should be created");

  let (sender, receiver) = mpsc::sync_channel(4);
  let cancel = Arc::new(AtomicBool::new(false));
  scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

  let mut entries = Vec::new();
  let mut completion = None;
  while let Ok(event) = receiver.try_recv() {
    match event {
      ScanEvent::Chunk(chunk) => entries.extend(chunk),
      ScanEvent::MetadataChunk(_) => {}
      ScanEvent::Finished { cacheable } => completion = Some(cacheable),
      ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
    }
  }

  assert!(entries.is_empty());
  assert_eq!(completion, Some(true));
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

  let (sender, receiver) = mpsc::sync_channel(8);
  let cancel = Arc::new(AtomicBool::new(false));
  scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

  let mut chunk_count = 0;
  let mut entry_count = 0;
  let mut finished = false;
  let mut cacheable = false;
  while let Ok(event) = receiver.try_recv() {
    match event {
      ScanEvent::Chunk(entries) => {
        chunk_count += 1;
        entry_count += entries.len();
      }
      ScanEvent::MetadataChunk(_) => {}
      ScanEvent::Finished {
        cacheable: finished_cacheable,
      } => {
        finished = true;
        cacheable = finished_cacheable;
      }
      ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
    }
  }

  assert_eq!(entry_count, CHUNK_SIZE + 1);
  assert_eq!(chunk_count, 2);
  assert!(finished);
  assert!(cacheable);
}

#[test]
fn cancellation_prevents_events() {
  let root = TemporaryDirectory::new();
  let (sender, receiver) = mpsc::sync_channel(4);
  let cancel = Arc::new(AtomicBool::new(true));
  scan_directory(root.0.clone(), sender, Arc::clone(&cancel), false);

  assert!(receiver.try_recv().is_err());
}

#[test]
fn includes_files_when_requested() {
  let root = TemporaryDirectory::new();
  fs::create_dir(root.0.join("directory")).expect("directory should be created");
  File::create(root.0.join("file.txt")).expect("file should be created");

  let (sender, receiver) = mpsc::sync_channel(4);
  let cancel = Arc::new(AtomicBool::new(false));
  scan_directory(root.0.clone(), sender, Arc::clone(&cancel), true);

  let mut entries = Vec::new();
  let mut metadata = Vec::new();
  while let Ok(event) = receiver.try_recv() {
    match event {
      ScanEvent::Chunk(chunk) => entries.extend(chunk),
      ScanEvent::MetadataChunk(updates) => metadata.extend(updates),
      ScanEvent::Finished { cacheable } => assert!(!cacheable),
      ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
    }
  }

  entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
  assert_eq!(entries.len(), 2);
  assert_eq!(entries[0].name, "directory");
  assert!(entries[0].is_directory);
  assert_eq!(entries[1].name, "file.txt");
  assert!(!entries[1].is_directory);
  assert_eq!(metadata.len(), 2);
  assert!(metadata.iter().all(|entry| entry.modified.is_some()));
}

#[test]
fn metadata_refresh_reports_unavailable_entries_without_failing() {
  let missing = env::temp_dir().join(format!(
    "fast-metadata-missing-{}-{}",
    std::process::id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock should be after the Unix epoch")
      .as_nanos()
  ));
  let (sender, receiver) = mpsc::sync_channel(2);
  let cancel = Arc::new(AtomicBool::new(false));
  refresh_metadata(vec![missing.clone()], sender, Arc::clone(&cancel));

  match receiver.try_recv().expect("metadata update should be sent") {
    ScanEvent::MetadataChunk(updates) => {
      assert_eq!(updates.len(), 1);
      assert_eq!(updates[0].path, missing);
      assert!(updates[0].modified.is_none());
    }
    event => panic!("unexpected event: {event:?}"),
  }
  assert!(matches!(
    receiver.try_recv(),
    Ok(ScanEvent::Finished { cacheable: false })
  ));
}

#[test]
fn metadata_refresh_is_chunked() {
  let paths = (0..(CHUNK_SIZE + 1))
    .map(|index| PathBuf::from(format!("/tmp/fast-metadata-{index}")))
    .collect::<Vec<_>>();
  let (sender, receiver) = mpsc::sync_channel(8);
  let cancel = Arc::new(AtomicBool::new(false));
  refresh_metadata(paths, sender, Arc::clone(&cancel));

  let mut chunk_count = 0;
  let mut entry_count = 0;
  let mut finished = false;
  while let Ok(event) = receiver.try_recv() {
    match event {
      ScanEvent::MetadataChunk(entries) => {
        chunk_count += 1;
        entry_count += entries.len();
      }
      ScanEvent::Finished { cacheable } => {
        finished = true;
        assert!(!cacheable);
      }
      event => panic!("unexpected event: {event:?}"),
    }
  }

  assert_eq!(chunk_count, 2);
  assert_eq!(entry_count, CHUNK_SIZE + 1);
  assert!(finished);
}

#[test]
fn metadata_cancellation_prevents_events() {
  let (sender, receiver) = mpsc::sync_channel(2);
  let cancel = Arc::new(AtomicBool::new(true));
  refresh_metadata(
    vec![PathBuf::from("/tmp/fast-metadata-cancelled")],
    sender,
    Arc::clone(&cancel),
  );

  assert!(receiver.try_recv().is_err());
}

#[test]
fn file_entries_are_emitted_in_chunks() {
  let root = TemporaryDirectory::new();
  for index in 0..(CHUNK_SIZE + 1) {
    File::create(root.0.join(format!("file-{index:03}.txt"))).expect("file should be created");
  }

  let (sender, receiver) = mpsc::sync_channel(8);
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
      ScanEvent::MetadataChunk(_) => {}
      ScanEvent::Finished {
        cacheable: finished_cacheable,
      } => {
        finished = true;
        assert!(!finished_cacheable);
      }
      ScanEvent::Error(error) => panic!("unexpected scan error: {error}"),
    }
  }

  assert_eq!(entry_count, CHUNK_SIZE + 1);
  assert_eq!(chunk_count, 2);
  assert!(finished);
}
