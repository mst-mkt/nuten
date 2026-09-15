//! Runs the `nu` on PATH.

use std::{
    io::Write,
    process::{Command, Stdio},
};

/// The series the formatter targets, as a prefix of `nu --version`.
const VERSION: &str = "0.115.";

/// Fails when nu cannot be run or is not of the series the formatter
/// targets.
pub(crate) fn check_version() -> Result<(), String> {
    let output = Command::new("nu")
        .arg("--version")
        .output()
        .map_err(|error| format!("nu --version: {error}"))?;
    if !output.status.success() {
        return Err(format!("nu --version fails ({})", output.status));
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let version = version.trim();
    if version.starts_with(VERSION) {
        Ok(())
    } else {
        Err(format!("nu {version} is not of the {VERSION}x series"))
    }
}

/// The block and the error nu parses `src` into, as one JSON text.
/// The block comes without its compiled IR, whose instruction order changes
/// from run to run. Fails, in one line, when nu does.
pub(crate) fn ast(src: &str) -> Result<String, String> {
    let mut nu = Command::new("nu")
        .args(["-n", "--stdin", "-c"])
        .arg(
            "ast $in --json | update block { from json | reject ir_block } \
             | update error { from json } | to json -r",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("nu: {error}"))?;
    // Nu reads all of `src` before it writes, so writing fails only when nu
    // has failed, which its status and stderr tell better.
    let written = nu
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(src.as_bytes());
    let output = nu
        .wait_with_output()
        .map_err(|error| format!("nu: {error}"))?;
    if !output.status.success() {
        // Nu writes the code of the error on one line and the message on a
        // later one.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message: Vec<_> = stderr
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(2)
            .collect();
        let status = format!("nu fails ({})", output.status);
        return Err(if message.is_empty() {
            status
        } else {
            format!("{status}: {}", message.join(" "))
        });
    }
    written.map_err(|error| format!("nu: {error}"))?;
    String::from_utf8(output.stdout).map_err(|error| format!("nu: {error}"))
}
