mod app;
mod cache;
mod cli;
mod filter;
mod scan;
mod terminal;

use std::{env, io};

use app::{App, ExitAction};
use cli::CliError;
use terminal::TerminalSession;

fn run() -> io::Result<i32> {
  let selection_file = match cli::parse_args(env::args_os().skip(1)) {
    Ok(selection_file) => selection_file,
    Err(CliError::Help) => {
      cli::print_help();
      return Ok(0);
    }
    Err(CliError::Version) => {
      cli::print_version();
      return Ok(0);
    }
    Err(CliError::Invalid(message)) => {
      eprintln!("fast: {message}");
      eprintln!("Try `fast --help` for usage.");
      return Ok(2);
    }
  };
  let current_dir = env::current_dir()?;
  let mut terminal = TerminalSession::enter()?;
  let mut app = App::new(current_dir);
  let action = app.run(terminal.output_mut())?;
  drop(terminal);

  match action {
    ExitAction::Select(selected_path) => {
      if let Some(selection_file) = selection_file {
        cli::write_selection_file(&selection_file, &selected_path)?;
      }
      Ok(0)
    }
    ExitAction::Cancel => Ok(if selection_file.is_some() { 1 } else { 0 }),
  }
}

fn main() {
  let code = match run() {
    Ok(code) => code,
    Err(error) => {
      eprintln!("fast: {error}");
      1
    }
  };
  if code != 0 {
    std::process::exit(code);
  }
}
