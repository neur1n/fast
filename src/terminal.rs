use std::io::{self, Stdout, stdout};

use crossterm::{
  cursor::{Hide, Show},
  execute,
  style::ResetColor,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

pub(crate) struct TerminalSession {
  output: Stdout,
}

impl TerminalSession {
  pub(crate) fn enter() -> io::Result<Self> {
    let mut output = stdout();
    enable_raw_mode()?;
    if let Err(error) = execute!(&mut output, EnterAlternateScreen, Hide) {
      let _ = disable_raw_mode();
      return Err(error);
    }
    Ok(Self { output })
  }

  pub(crate) fn output_mut(&mut self) -> &mut Stdout {
    &mut self.output
  }
}

impl Drop for TerminalSession {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
    let _ = execute!(&mut self.output, Show, LeaveAlternateScreen, ResetColor);
  }
}
