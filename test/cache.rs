use std::{
  fs,
  sync::{Arc, Mutex},
  thread,
};

use super::*;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
  fn new() -> Self {
    let path = std::env::temp_dir().join(format!(
      "fast-cache-test-{}-{}",
      std::process::id(),
      NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
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

fn directory_entry(directory: &Path, name: &str) -> DirectoryEntry {
  DirectoryEntry {
    name: name.to_owned(),
    path: directory.join(name),
    is_directory: true,
  }
}

fn store_current(cache: &DirectoryCache, directory: &Path, entries: &[DirectoryEntry]) {
  let fingerprint = DirectoryCache::fingerprint(directory).unwrap();
  assert!(
    cache
      .store_if_unchanged(directory, &fingerprint, entries)
      .unwrap()
  );
}

#[test]
fn stores_and_loads_matching_directory() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let child = directory.join("child");
  fs::create_dir(&child).expect("child should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  let entries = vec![directory_entry(&directory, "child")];

  store_current(&cache, &directory, &entries);

  assert_eq!(cache.load(&directory).unwrap(), Some(entries));
}

#[test]
fn rejects_a_record_with_a_different_fingerprint() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  let record = CacheRecord {
    directory: directory.clone(),
    fingerprint: DirectoryFingerprint {
      modified_seconds: fingerprint.modified_seconds.saturating_add(1),
      ..fingerprint
    },
    entries: Vec::new(),
  };
  fs::create_dir_all(&cache.root).unwrap();
  fs::write(
    cache.record_path(&directory).unwrap(),
    encode_record(&record).unwrap(),
  )
  .unwrap();

  assert_eq!(cache.load(&directory).unwrap(), None);
}

#[test]
fn detects_corrupted_records() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  fs::create_dir_all(&cache.root).unwrap();
  fs::write(cache.record_path(&directory).unwrap(), b"not a cache").unwrap();

  let error = cache
    .load(&directory)
    .expect_err("corruption should be reported");
  assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn rejects_legacy_cache_records() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
  let record = CacheRecord {
    directory: directory.clone(),
    fingerprint,
    entries: Vec::new(),
  };
  let mut bytes = encode_record(&record).unwrap();
  bytes[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&1u32.to_le_bytes());
  fs::create_dir_all(&cache.root).unwrap();
  fs::write(cache.record_path(&directory).unwrap(), bytes).unwrap();

  let error = cache
    .load(&directory)
    .expect_err("legacy cache records should be rejected");
  assert!(error.to_string().contains("unsupported version"));
}

#[test]
fn rejects_oversized_records() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  fs::create_dir_all(&cache.root).unwrap();
  fs::write(
    cache.record_path(&directory).unwrap(),
    vec![0; MAX_RECORD_BYTES + 1],
  )
  .unwrap();

  let error = cache
    .load(&directory)
    .expect_err("oversized records should be rejected");
  assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn does_not_write_when_scan_fingerprint_changed() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  let before = DirectoryCache::fingerprint(&directory).unwrap();
  let changed = DirectoryFingerprint {
    modified_seconds: before.modified_seconds.saturating_add(1),
    ..before
  };

  assert!(!cache.store_if_unchanged(&directory, &changed, &[]).unwrap());
  assert_eq!(cache.load(&directory).unwrap(), None);
}

#[test]
fn replaces_a_complete_record_without_leaving_temporary_data() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = DirectoryCache::new(root.0.join("cache"));
  let first = vec![directory_entry(&directory, "first")];
  let second = vec![directory_entry(&directory, "second")];

  store_current(&cache, &directory, &first);
  store_current(&cache, &directory, &second);

  assert_eq!(cache.load(&directory).unwrap(), Some(second));
  let has_temporary_file = fs::read_dir(&cache.root)
    .unwrap()
    .filter_map(Result::ok)
    .any(|entry| entry.file_name().to_string_lossy().contains(".cache.tmp-"));
  assert!(!has_temporary_file);
}

#[test]
fn concurrent_writers_leave_a_valid_record() {
  let root = TemporaryDirectory::new();
  let directory = root.0.join("workspace");
  fs::create_dir(&directory).expect("workspace should be created");
  let cache = Arc::new(DirectoryCache::new(root.0.join("cache")));
  let entries = vec![directory_entry(&directory, "child")];
  let errors = Arc::new(Mutex::new(Vec::new()));
  let mut workers = Vec::new();
  for _ in 0..8 {
    let cache = Arc::clone(&cache);
    let directory = directory.clone();
    let entries = entries.clone();
    let errors = Arc::clone(&errors);
    workers.push(thread::spawn(move || {
      let fingerprint = DirectoryCache::fingerprint(&directory).unwrap();
      if let Err(error) = cache.store_if_unchanged(&directory, &fingerprint, &entries) {
        errors.lock().unwrap().push(error.to_string());
      }
    }));
  }
  for worker in workers {
    worker.join().expect("cache writer should finish");
  }

  assert!(errors.lock().unwrap().is_empty());
  assert_eq!(cache.load(&directory).unwrap(), Some(entries));
}

#[test]
fn bounds_the_number_of_cache_files() {
  let root = TemporaryDirectory::new();
  let cache = DirectoryCache::new(root.0.join("cache"));
  for index in 0..=MAX_CACHE_FILES {
    let directory = root.0.join(format!("workspace-{index}"));
    fs::create_dir(&directory).expect("workspace should be created");
    store_current(&cache, &directory, &[]);
  }

  let count = fs::read_dir(&cache.root)
    .unwrap()
    .filter_map(Result::ok)
    .filter(|entry| {
      entry
        .path()
        .extension()
        .is_some_and(|extension| extension == "cache")
    })
    .count();
  assert!(count <= MAX_CACHE_FILES);
}
