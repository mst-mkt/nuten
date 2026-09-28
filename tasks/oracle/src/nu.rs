//! Runs the `nu` on PATH.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

/// The series the formatter targets, as a prefix of `nu --version`.
const VERSION: &str = "0.116.";

/// Fails when nu cannot be run or is not of the series the formatter targets.
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

/// The block and the error nu parses `src` into, and its flattened tokens, as one JSON text.
/// The block comes without its compiled IR, whose instruction order changes from run to run.
/// Nu runs in `dir`, if given, so that it finds the modules `src` uses by relative path.
/// Fails, in one line, when nu does.
pub(crate) fn ast(src: &str, dir: Option<&Path>) -> Result<String, String> {
    run(
        "let src = $in; ast $src --json | update block { from json | reject ir_block } \
         | update error { from json } | insert flat { ast $src --flatten | reject span } \
         | to json -r",
        src,
        dir,
    )
}

/// The kind of the error nu reports on parsing `src`, or `None` when it reports none.
/// Nu writes the error as an object whose one key names the kind.
/// A kind can be about a name rather than the syntax, as `VariableNotFound` is.
/// Fails, in one line, when nu does.
pub(crate) fn parse_error(src: &str, dir: Option<&Path>) -> Result<Option<String>, String> {
    let kind = run(
        "let src = $in; ast $src --json | get error | from json \
         | if ($in | is-empty) { '' } else { $in | columns | first }",
        src,
        dir,
    )?;
    let kind = kind.trim();
    Ok((!kind.is_empty()).then(|| kind.to_string()))
}

/// The value nu reads from the NUON `src`, described with its types, as JSON text.
/// `describe -d` writes each value as an object with its `type`.
/// Beside it stand a scalar's `value`, a record's `columns`, or a list's items as `value`.
/// Fails, in one line, when nu does.
pub(crate) fn from_nuon(src: &str) -> Result<String, String> {
    run("from nuon | describe -d | to json -r", src, None)
}

/// The value nu reads from the NUON `src`, written as NUON again.
/// Unlike JSON, NUON tells a date from a string and a filesize from an integer.
/// Fails, in one line, when nu does.
pub(crate) fn value_as_nuon(src: &str) -> Result<String, String> {
    // The line break nu prints after the NUON is no part of the value.
    run("from nuon | to nuon", src, None)
        .map(|written| written.strip_suffix('\n').unwrap_or(&written).to_string())
}

/// What `script` writes to stdout with `src` as its input.
/// Nu runs in `dir`, if given.
/// Fails, in one line, when nu does.
fn run(script: &str, src: &str, dir: Option<&Path>) -> Result<String, String> {
    let mut nu = Command::new("nu");
    if let Some(dir) = dir {
        nu.current_dir(dir);
    }
    let mut nu = nu
        .args(["-n", "--stdin", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("nu: {error}"))?;
    // Nu reads all of `src` before it writes, so writing fails only when nu has failed.
    // Its status and stderr tell that better.
    let written = nu
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(src.as_bytes());
    let output = nu
        .wait_with_output()
        .map_err(|error| format!("nu: {error}"))?;
    if !output.status.success() {
        // Nu writes the code of the error on one line and the message on a later one.
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
