use crate::scan::DirectoryEntry;

const CONTIGUOUS_BONUS: i64 = 20;
const WORD_BOUNDARY_BONUS: i64 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FilterKind {
  Substring,
  Fuzzy,
}

impl FilterKind {
  pub(crate) fn label(self) -> &'static str {
    match self {
      Self::Substring => "Simple",
      Self::Fuzzy => "Fuzzy",
    }
  }

  pub(crate) fn toggle(self) -> Self {
    match self {
      Self::Substring => Self::Fuzzy,
      Self::Fuzzy => Self::Substring,
    }
  }
}

pub(crate) fn matching_indices(entries: &[DirectoryEntry], query: &str) -> Vec<usize> {
  let query = query.to_lowercase();
  entries
    .iter()
    .enumerate()
    .filter_map(|(index, entry)| {
      (is_navigation_entry(entry) || query.is_empty() || entry.name.to_lowercase().contains(&query))
        .then_some(index)
    })
    .collect()
}

pub(crate) fn fuzzy_indices(entries: &[DirectoryEntry], query: &str) -> Vec<usize> {
  if query.is_empty() {
    return (0..entries.len()).collect();
  }

  let mut navigation_indices = Vec::new();
  let mut matches = Vec::new();
  for (index, entry) in entries.iter().enumerate() {
    if is_navigation_entry(entry) {
      navigation_indices.push(index);
      continue;
    }
    if let Some(score) = fuzzy_score(&entry.name, query) {
      matches.push((score, index));
    }
  }

  matches.sort_unstable_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
  navigation_indices.extend(matches.into_iter().map(|(_, index)| index));
  navigation_indices
}

fn is_navigation_entry(entry: &DirectoryEntry) -> bool {
  entry.name == "." || entry.name == ".."
}

fn fuzzy_score(name: &str, query: &str) -> Option<i64> {
  let name = name.to_lowercase().chars().collect::<Vec<_>>();
  let query = query.to_lowercase().chars().collect::<Vec<_>>();
  if query.is_empty() {
    return Some(0);
  }

  let mut score = 0;
  let mut search_start = 0;
  let mut previous_position = None;
  for query_character in query {
    let position = name
      .iter()
      .enumerate()
      .skip(search_start)
      .find(|(_, name_character)| **name_character == query_character)
      .map(|(position, _)| position)?;

    score += (name.len() - position) as i64;
    if position == 0 || !name[position - 1].is_alphanumeric() {
      score += WORD_BOUNDARY_BONUS;
    }
    if previous_position == Some(position.saturating_sub(1)) {
      score += CONTIGUOUS_BONUS;
    }
    previous_position = Some(position);
    search_start = position + 1;
  }

  Some(score - name.len() as i64)
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
  fn fuzzy_matching_keeps_parent_visible_and_rejects_missing_matches() {
    let entries = [entry(".."), entry("."), entry("alpha")];

    assert_eq!(fuzzy_indices(&entries, "zz"), vec![0, 1]);
  }

  #[test]
  fn fuzzy_matching_keeps_ties_in_input_order() {
    let entries = [entry("alpha"), entry("alpha")];

    assert_eq!(fuzzy_indices(&entries, "ap"), vec![0, 1]);
  }
}
