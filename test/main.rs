use super::*;

#[cfg(unix)]
#[test]
fn preserves_a_logical_path_for_a_symlinked_directory() {
  use std::{
    fs,
    os::unix::fs::symlink,
    time::{SystemTime, UNIX_EPOCH},
  };

  let root = env::temp_dir().join(format!(
    "fast-main-test-{}-{}",
    std::process::id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos()
  ));
  let physical = root.join("A").join("foo1").join("bar1");
  let logical = root.join("B").join("foo2").join("bar1");
  fs::create_dir_all(&physical).unwrap();
  fs::create_dir_all(logical.parent().unwrap()).unwrap();
  symlink(&physical, &logical).unwrap();

  assert_eq!(
    prefer_logical_path(&physical, Some(logical.clone())),
    logical
  );

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn ignores_a_logical_path_that_does_not_resolve_to_the_current_directory() {
  use std::fs;

  let root = env::temp_dir().join(format!(
    "fast-main-test-{}-{}",
    std::process::id(),
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos()
  ));
  let physical = root.join("physical");
  let other = root.join("other");
  fs::create_dir_all(&physical).unwrap();
  fs::create_dir_all(&other).unwrap();

  assert_eq!(prefer_logical_path(&physical, Some(other)), physical);
  assert_eq!(
    prefer_logical_path(&physical, Some(PathBuf::from("relative"))),
    physical
  );

  fs::remove_dir_all(root).unwrap();
}
