//! Small helpers for shelling out to system tools.

use std::env;
use std::process::{Command, Output};

/// True when `command` exists in `PATH`.
pub fn exists(command: &str) -> bool {
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(command).is_file()))
        .unwrap_or(false)
}

/// Run a command and return its trimmed stdout. Errors include stderr when
/// the tool reported one.
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    output_result(program, output)
}

/// Same as [`run`], for argument lists built as owned strings.
pub fn run_owned(program: &str, args: &[String]) -> Result<String, String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run(program, &refs)
}

fn output_result(program: &str, output: Output) -> Result<String, String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("{program} exited with status {}", output.status))
    } else {
        Err(stderr)
    }
}
