use std::{
  ffi::OsString,
  fs, io,
  path::{Path, PathBuf},
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CliError {
  Help,
  Invalid(String),
}

pub(crate) fn parse_args<I>(args: I) -> Result<Option<PathBuf>, CliError>
where
  I: IntoIterator<Item = OsString>,
{
  let mut selection_file = None;
  let mut args = args.into_iter();
  while let Some(argument) = args.next() {
    let Some(argument) = argument.to_str() else {
      return Err(CliError::Invalid(
        "arguments must be valid UTF-8".to_owned(),
      ));
    };
    match argument {
      "-h" | "--help" => return Err(CliError::Help),
      "--select" => {
        if selection_file.is_some() {
          return Err(CliError::Invalid(
            "--select may only be provided once".to_owned(),
          ));
        }
        let Some(path) = args.next() else {
          return Err(CliError::Invalid("--select requires a path".to_owned()));
        };
        if path.is_empty() {
          return Err(CliError::Invalid(
            "--select requires a non-empty path".to_owned(),
          ));
        }
        selection_file = Some(PathBuf::from(path));
      }
      argument => {
        return Err(CliError::Invalid(format!("unknown argument: {argument}")));
      }
    }
  }
  Ok(selection_file)
}

pub(crate) fn selection_bytes(path: &Path) -> io::Result<Vec<u8>> {
  let path = path.to_str().ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      "selected path is not valid UTF-8",
    )
  })?;
  let mut bytes = Vec::with_capacity(path.len() + 1);
  bytes.extend_from_slice(path.as_bytes());
  bytes.push(0);
  Ok(bytes)
}

pub(crate) fn write_selection_file(selection_file: &Path, selected_path: &Path) -> io::Result<()> {
  fs::write(selection_file, selection_bytes(selected_path)?)
}

pub(crate) fn print_help() {
  println!("Usage: fast [--select PATH]");
  println!();
  println!("Browse directories from the current working directory.");
  println!("  --select PATH  write the selected directory on confirmation");
  println!();
  println!("  q          select the highlighted directory");
  println!("  /          filter names (fuzzy by default; Tab toggles simple/fuzzy)");
  println!("  Esc/Ctrl-C  cancel without selecting a directory");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_select_argument() {
    let args = [OsString::from("--select"), OsString::from("result")];
    assert_eq!(parse_args(args), Ok(Some(PathBuf::from("result"))));
  }

  #[test]
  fn selection_protocol_is_nul_terminated() {
    assert_eq!(
      selection_bytes(Path::new("/tmp/selected\npath")).unwrap(),
      b"/tmp/selected\npath\0"
    );
  }
}
