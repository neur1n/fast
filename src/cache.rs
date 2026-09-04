use std::{
  env,
  fs::{self, File, OpenOptions},
  io::{self, ErrorKind, Read, Write},
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::scan::DirectoryEntry;

const MAGIC: &[u8; 8] = b"FASTCACH";
const FORMAT_VERSION: u32 = 1;
const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const MAX_CACHE_FILES: usize = 256;
const MAX_CACHE_BYTES: u64 = 16 * 1024 * 1024;
const TEMP_FILE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryFingerprint {
  modified_seconds: u64,
  modified_nanos: u32,
  length: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryCache {
  root: PathBuf,
}

struct CacheRecord {
  directory: PathBuf,
  fingerprint: DirectoryFingerprint,
  entries: Vec<DirectoryEntry>,
}

impl DirectoryCache {
  pub(crate) fn new(root: PathBuf) -> Self {
    Self { root }
  }

  pub(crate) fn system() -> Option<Self> {
    if let Some(root) = env::var_os("FAST_CACHE_DIR") {
      let root = PathBuf::from(root);
      if !root.as_os_str().is_empty() {
        return Some(Self::new(root));
      }
    }

    #[cfg(windows)]
    let base = env::var_os("LOCALAPPDATA").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME").map(|home| PathBuf::from(home).join("Library").join("Caches"));

    #[cfg(all(not(windows), not(target_os = "macos")))]
    let base = env::var_os("XDG_CACHE_HOME")
      .map(PathBuf::from)
      .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));

    base.map(|base| Self::new(base.join("fast")))
  }

  pub(crate) fn fingerprint(directory: &Path) -> io::Result<DirectoryFingerprint> {
    let metadata = fs::metadata(directory)?;
    if !metadata.is_dir() {
      return Err(io::Error::new(
        ErrorKind::NotADirectory,
        format!("{} is not a directory", directory.display()),
      ));
    }
    let modified = metadata.modified()?;
    let elapsed = modified.duration_since(UNIX_EPOCH).map_err(|_| {
      io::Error::new(
        ErrorKind::InvalidData,
        "directory modification time is before the Unix epoch",
      )
    })?;
    Ok(DirectoryFingerprint {
      modified_seconds: elapsed.as_secs(),
      modified_nanos: elapsed.subsec_nanos(),
      length: metadata.len(),
    })
  }

  pub(crate) fn load(&self, directory: &Path) -> io::Result<Option<Vec<DirectoryEntry>>> {
    let fingerprint = Self::fingerprint(directory)?;
    let path = self.record_path(directory)?;
    let file = match File::open(path) {
      Ok(file) => file,
      Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file
      .take((MAX_RECORD_BYTES + 1) as u64)
      .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD_BYTES {
      return Err(invalid_cache("cache record is too large"));
    }
    let record = decode_record(&bytes)?;
    if record.directory != directory || record.fingerprint != fingerprint {
      return Ok(None);
    }
    if record
      .entries
      .iter()
      .any(|entry| entry.path.parent() != Some(directory))
    {
      return Err(invalid_cache("cache contains a non-child directory"));
    }
    if record.entries.iter().any(|entry| !entry.is_directory) {
      return Err(invalid_cache("cache contains a non-directory entry"));
    }
    Ok(Some(record.entries))
  }

  pub(crate) fn store_if_unchanged(
    &self,
    directory: &Path,
    before: &DirectoryFingerprint,
    entries: &[DirectoryEntry],
  ) -> io::Result<bool> {
    let after = Self::fingerprint(directory)?;
    if &after != before {
      return Ok(false);
    }
    self.store_record(directory, after, entries)?;
    Ok(true)
  }

  fn store_record(
    &self,
    directory: &Path,
    fingerprint: DirectoryFingerprint,
    entries: &[DirectoryEntry],
  ) -> io::Result<()> {
    let record = CacheRecord {
      directory: directory.to_path_buf(),
      fingerprint,
      entries: entries.to_vec(),
    };
    let bytes = encode_record(&record)?;
    fs::create_dir_all(&self.root)?;
    let target = self.record_path(directory)?;
    write_atomically(&target, &bytes)?;
    self.enforce_limits()
  }

  fn record_path(&self, directory: &Path) -> io::Result<PathBuf> {
    let directory = directory.to_str().ok_or_else(|| {
      io::Error::new(
        ErrorKind::InvalidData,
        "cache only supports UTF-8 directory paths",
      )
    })?;
    Ok(
      self
        .root
        .join(format!("{:016x}.cache", fnv1a(directory.as_bytes()))),
    )
  }

  fn enforce_limits(&self) -> io::Result<()> {
    let directory = match fs::read_dir(&self.root) {
      Ok(directory) => directory,
      Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
      Err(error) => return Err(error),
    };

    let now = SystemTime::now();
    let mut records = Vec::new();
    for result in directory {
      let entry = match result {
        Ok(entry) => entry,
        Err(error) if error.kind() == ErrorKind::NotFound => continue,
        Err(error) => return Err(error),
      };
      let path = entry.path();
      let name = entry.file_name();
      let Some(name) = name.to_str() else {
        continue;
      };
      let metadata = match entry.metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => continue,
        Err(error) => return Err(error),
      };

      if name.ends_with(".cache") && metadata.is_file() {
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        records.push((path, metadata.len(), modified));
      } else if name.contains(".cache.tmp-")
        && metadata.is_file()
        && now
          .duration_since(metadata.modified().unwrap_or(UNIX_EPOCH))
          .is_ok_and(|age| age > TEMP_FILE_MAX_AGE)
      {
        let _ = fs::remove_file(path);
      }
    }

    let mut total_bytes = records
      .iter()
      .fold(0u64, |total, record| total.saturating_add(record.1));
    records.sort_unstable_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));
    while (records.len() > MAX_CACHE_FILES || total_bytes > MAX_CACHE_BYTES) && !records.is_empty()
    {
      let (path, size, _) = records.remove(0);
      match fs::remove_file(path) {
        Ok(()) => total_bytes = total_bytes.saturating_sub(size),
        Err(error) if error.kind() == ErrorKind::NotFound => {
          total_bytes = total_bytes.saturating_sub(size)
        }
        Err(error) => return Err(error),
      }
    }
    Ok(())
  }
}

fn write_atomically(target: &Path, bytes: &[u8]) -> io::Result<()> {
  let temp = temporary_path(target);
  let result = (|| {
    let mut file = OpenOptions::new()
      .create_new(true)
      .write(true)
      .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    replace_file(&temp, target)
  })();
  if result.is_err() {
    let _ = fs::remove_file(&temp);
  }
  result
}

fn temporary_path(target: &Path) -> PathBuf {
  let counter = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
  let name = target
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("cache");
  target.with_file_name(format!("{name}.tmp-{}-{counter}", std::process::id()))
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
  fs::rename(temp, target)
}

#[cfg(windows)]
fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
  match fs::rename(temp, target) {
    Ok(()) => Ok(()),
    Err(rename_error) => match fs::remove_file(target) {
      Ok(()) => fs::rename(temp, target),
      Err(error) if error.kind() == ErrorKind::NotFound => fs::rename(temp, target),
      Err(_) => Err(rename_error),
    },
  }
}

fn encode_record(record: &CacheRecord) -> io::Result<Vec<u8>> {
  if record.entries.len() > MAX_ENTRIES {
    return Err(invalid_cache("too many directory entries"));
  }

  let mut body = Vec::new();
  push_path(&mut body, &record.directory)?;
  push_u64(&mut body, record.fingerprint.modified_seconds);
  push_u32(&mut body, record.fingerprint.modified_nanos);
  push_u64(&mut body, record.fingerprint.length);
  push_u32(
    &mut body,
    u32::try_from(record.entries.len()).map_err(|_| invalid_cache("too many directory entries"))?,
  );
  for entry in &record.entries {
    if entry.path.parent() != Some(record.directory.as_path()) {
      return Err(invalid_cache("cache entry is not a direct child"));
    }
    if !entry.is_directory {
      return Err(invalid_cache("cache entry is not a directory"));
    }
    push_string(&mut body, &entry.name)?;
    push_path(&mut body, &entry.path)?;
  }

  let total_length = 8usize
    .checked_add(4)
    .and_then(|length| length.checked_add(4))
    .and_then(|length| length.checked_add(body.len()))
    .and_then(|length| length.checked_add(8))
    .ok_or_else(|| invalid_cache("cache record is too large"))?;
  if total_length > MAX_RECORD_BYTES {
    return Err(invalid_cache("cache record is too large"));
  }

  let mut bytes = Vec::with_capacity(total_length);
  bytes.extend_from_slice(MAGIC);
  push_u32(&mut bytes, FORMAT_VERSION);
  push_u32(
    &mut bytes,
    u32::try_from(body.len()).map_err(|_| invalid_cache("cache record is too large"))?,
  );
  bytes.extend_from_slice(&body);
  push_u64(&mut bytes, fnv1a(&body));
  Ok(bytes)
}

fn decode_record(bytes: &[u8]) -> io::Result<CacheRecord> {
  const PREFIX_LENGTH: usize = 8 + 4 + 4;
  const MINIMUM_LENGTH: usize = PREFIX_LENGTH + 8;
  if bytes.len() > MAX_RECORD_BYTES || bytes.len() < MINIMUM_LENGTH {
    return Err(invalid_cache("cache record has an invalid size"));
  }
  if &bytes[..MAGIC.len()] != MAGIC {
    return Err(invalid_cache("cache record has an invalid magic value"));
  }

  let version = read_u32(&bytes[MAGIC.len()..])?;
  if version != FORMAT_VERSION {
    return Err(invalid_cache("cache record has an unsupported version"));
  }
  let body_length = read_u32(&bytes[MAGIC.len() + 4..])? as usize;
  let body_start = PREFIX_LENGTH;
  let checksum_start = body_start
    .checked_add(body_length)
    .ok_or_else(|| invalid_cache("cache record length overflow"))?;
  if checksum_start.checked_add(8) != Some(bytes.len()) {
    return Err(invalid_cache("cache record has an invalid body length"));
  }
  let body = &bytes[body_start..checksum_start];
  let checksum = read_u64(&bytes[checksum_start..])?;
  if checksum != fnv1a(body) {
    return Err(invalid_cache("cache record checksum mismatch"));
  }

  let mut reader = Reader::new(body);
  let directory = PathBuf::from(reader.string()?);
  let fingerprint = DirectoryFingerprint {
    modified_seconds: reader.u64()?,
    modified_nanos: reader.u32()?,
    length: reader.u64()?,
  };
  let entry_count = reader.u32()? as usize;
  if entry_count > MAX_ENTRIES {
    return Err(invalid_cache("cache record contains too many entries"));
  }
  let mut entries = Vec::with_capacity(entry_count);
  for _ in 0..entry_count {
    entries.push(DirectoryEntry {
      name: reader.string()?,
      path: PathBuf::from(reader.string()?),
      is_directory: true,
    });
  }
  if !reader.is_empty() {
    return Err(invalid_cache("cache record has trailing data"));
  }
  Ok(CacheRecord {
    directory,
    fingerprint,
    entries,
  })
}

fn push_path(buffer: &mut Vec<u8>, path: &Path) -> io::Result<()> {
  let path = path
    .to_str()
    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "cache only supports UTF-8 paths"))?;
  push_string(buffer, path)
}

fn push_string(buffer: &mut Vec<u8>, value: &str) -> io::Result<()> {
  let length = u32::try_from(value.len()).map_err(|_| invalid_cache("cache string is too long"))?;
  let new_length = buffer
    .len()
    .checked_add(4)
    .and_then(|length| length.checked_add(value.len()))
    .ok_or_else(|| invalid_cache("cache record is too large"))?;
  if new_length > MAX_RECORD_BYTES {
    return Err(invalid_cache("cache record is too large"));
  }
  push_u32(buffer, length);
  buffer.extend_from_slice(value.as_bytes());
  Ok(())
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
  buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(buffer: &mut Vec<u8>, value: u64) {
  buffer.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8]) -> io::Result<u32> {
  let bytes = bytes
    .get(..4)
    .ok_or_else(|| invalid_cache("cache record ended unexpectedly"))?;
  Ok(u32::from_le_bytes(
    bytes.try_into().expect("slice length checked"),
  ))
}

fn read_u64(bytes: &[u8]) -> io::Result<u64> {
  let bytes = bytes
    .get(..8)
    .ok_or_else(|| invalid_cache("cache record ended unexpectedly"))?;
  Ok(u64::from_le_bytes(
    bytes.try_into().expect("slice length checked"),
  ))
}

struct Reader<'a> {
  bytes: &'a [u8],
  offset: usize,
}

impl<'a> Reader<'a> {
  fn new(bytes: &'a [u8]) -> Self {
    Self { bytes, offset: 0 }
  }

  fn take(&mut self, length: usize) -> io::Result<&'a [u8]> {
    let end = self
      .offset
      .checked_add(length)
      .ok_or_else(|| invalid_cache("cache record length overflow"))?;
    let bytes = self
      .bytes
      .get(self.offset..end)
      .ok_or_else(|| invalid_cache("cache record ended unexpectedly"))?;
    self.offset = end;
    Ok(bytes)
  }

  fn u32(&mut self) -> io::Result<u32> {
    read_u32(self.take(4)?)
  }

  fn u64(&mut self) -> io::Result<u64> {
    read_u64(self.take(8)?)
  }

  fn string(&mut self) -> io::Result<String> {
    let length = self.u32()? as usize;
    let bytes = self.take(length)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| invalid_cache("cache contains invalid UTF-8"))
  }

  fn is_empty(&self) -> bool {
    self.offset == self.bytes.len()
  }
}

fn invalid_cache(message: &str) -> io::Error {
  io::Error::new(ErrorKind::InvalidData, message)
}

fn fnv1a(bytes: &[u8]) -> u64 {
  let mut hash = 0xcbf29ce484222325;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x100000001b3);
  }
  hash
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    fs,
    sync::{Arc, Mutex},
    thread,
  };

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
}
