use crate::scan::DirectoryEntry;

pub(crate) fn matching_indices(entries: &[DirectoryEntry], query: &str) -> Vec<usize> {
  let query = query.to_lowercase();
  entries
    .iter()
    .enumerate()
    .filter_map(|(index, entry)| {
      (entry.name == ".." || query.is_empty() || entry.name.to_lowercase().contains(&query))
        .then_some(index)
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn entry(name: &str) -> DirectoryEntry {
    DirectoryEntry {
      name: name.to_owned(),
      path: PathBuf::from("/").join(name),
    }
  }

  #[test]
  fn matches_case_insensitive_literal_substrings() {
    let entries = [entry("alpha"), entry("Beta"), entry("gamma")];

    assert_eq!(matching_indices(&entries, "ET"), vec![1]);
  }

  #[test]
  fn keeps_parent_entry_visible() {
    let entries = [entry(".."), entry("alpha")];

    assert_eq!(matching_indices(&entries, "missing"), vec![0]);
  }

  #[test]
  fn empty_query_matches_every_entry() {
    let entries = [entry(".."), entry("alpha"), entry("beta")];

    assert_eq!(matching_indices(&entries, ""), vec![0, 1, 2]);
  }
}
