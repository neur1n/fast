use crate::scan::DirectoryEntry;

const CONTIGUOUS_BONUS: i64 = 20;
const WORD_BOUNDARY_BONUS: i64 = 12;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FilterKind {
  #[default]
  Fuzzy,
  Substring,
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
      matches.push((entry.is_directory, score, index));
    }
  }

  matches.sort_unstable_by(|left, right| {
    right
      .0
      .cmp(&left.0)
      .then_with(|| right.1.cmp(&left.1))
      .then_with(|| left.2.cmp(&right.2))
  });
  navigation_indices.extend(matches.into_iter().map(|(_, _, index)| index));
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
#[path = "../test/filter.rs"]
mod tests;
