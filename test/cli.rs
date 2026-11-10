use super::*;

#[test]
fn parses_select_argument() {
  let args = [OsString::from("--select"), OsString::from("result")];
  assert_eq!(parse_args(args), Ok(Some(PathBuf::from("result"))));
}

#[test]
fn parses_version_arguments() {
  assert_eq!(
    parse_args([OsString::from("--version")]),
    Err(CliError::Version)
  );
  assert_eq!(parse_args([OsString::from("-V")]), Err(CliError::Version));
}

#[test]
fn build_version_is_not_empty() {
  assert!(!BUILD_VERSION.is_empty());
}

#[test]
fn selection_protocol_is_nul_terminated() {
  assert_eq!(
    selection_bytes(Path::new("/tmp/selected\npath")).unwrap(),
    b"/tmp/selected\npath\0"
  );
}
