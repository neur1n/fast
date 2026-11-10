use std::path::PathBuf;

use super::*;

fn entry(name: &str) -> DirectoryEntry {
  entry_with_kind(name, true)
}

fn entry_with_kind(name: &str, is_directory: bool) -> DirectoryEntry {
  DirectoryEntry {
    name: name.to_owned(),
    path: PathBuf::from("/").join(name),
    is_directory,
  }
}

#[test]
fn matches_case_insensitive_literal_substrings() {
  let entries = [entry("alpha"), entry("Beta"), entry("gamma")];

  assert_eq!(matching_indices(&entries, "ET"), vec![1]);
}

#[test]
fn defaults_to_fuzzy_matching() {
  assert_eq!(FilterKind::default(), FilterKind::Fuzzy);
}

#[test]
fn keeps_navigation_entries_visible() {
  let entries = [entry(".."), entry("."), entry("alpha")];

  assert_eq!(matching_indices(&entries, "missing"), vec![0, 1]);
  assert_eq!(fuzzy_indices(&entries, "missing"), vec![0, 1]);
}

#[test]
fn empty_query_matches_every_entry() {
  let entries = [entry(".."), entry("."), entry("alpha"), entry("beta")];

  assert_eq!(matching_indices(&entries, ""), vec![0, 1, 2, 3]);
}

#[test]
fn fuzzy_matches_characters_in_order() {
  let entries = [entry("target"), entry("team"), entry("tree")];

  assert_eq!(fuzzy_indices(&entries, "tm"), vec![1]);
  assert_eq!(fuzzy_indices(&entries, "rt"), vec![0]);
}

#[test]
fn fuzzy_matching_is_case_insensitive() {
  let entries = [entry("ProjectRoot")];

  assert_eq!(fuzzy_indices(&entries, "pr"), vec![0]);
}

#[test]
fn fuzzy_matching_prefers_contiguous_matches() {
  let entries = [entry("a-b"), entry("ab")];

  assert_eq!(fuzzy_indices(&entries, "ab"), vec![1, 0]);
}

#[test]
fn fuzzy_matching_groups_directories_before_files() {
  let entries = [
    entry_with_kind("a-file", false),
    entry_with_kind("z-directory", true),
  ];

  assert_eq!(fuzzy_indices(&entries, "e"), vec![1, 0]);
}

#[test]
fn fuzzy_matching_keeps_parent_visible_and_rejects_missing_matches() {
  let entries = [entry(".."), entry("."), entry("alpha")];

  assert_eq!(fuzzy_indices(&entries, "zz"), vec![0, 1]);
}

#[test]
fn fuzzy_matching_keeps_ties_in_input_order() {
  let entries = [entry("alpha"), entry("alpha")];

  assert_eq!(fuzzy_indices(&entries, "ap"), vec![0, 1]);
}
