use std::{fs, iter, process::Output};

use serde_json::{Value, json};

use crate::support::{BLOCK, TempDir, nuten_command, output_in_time, stderr, stdout};

fn lsp_in(dir: &TempDir, args: &[&str], messages: &[Value]) -> Output {
    let input: Vec<u8> = messages
        .iter()
        .flat_map(|message| {
            let body = message.to_string();
            format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
        })
        .collect();
    let input_path = dir.path("stdin.input");
    fs::write(&input_path, input).unwrap();
    output_in_time(
        nuten_command()
            .arg("lsp")
            .args(args)
            .current_dir(&dir.0)
            .stdin(fs::File::open(&input_path).unwrap()),
    )
}

fn lsp_messages(output: &Output) -> Vec<Value> {
    let stdout = stdout(output);
    let mut rest = stdout.as_str();
    iter::from_fn(|| {
        let (header, after) = rest.split_once("\r\n\r\n")?;
        let length = header.strip_prefix("Content-Length: ").unwrap();
        let (body, after) = after.split_at(length.parse().unwrap());
        rest = after;
        Some(serde_json::from_str(body).unwrap())
    })
    .collect()
}

fn lsp_request(id: i32, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn lsp_notification(method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

fn lsp_initialization() -> [Value; 2] {
    [
        lsp_request(0, "initialize", &json!({ "capabilities": {} })),
        lsp_notification("initialized", &json!({})),
    ]
}

fn lsp_opening(uri: &str, text: &str) -> Value {
    let document = json!({ "uri": uri, "languageId": "nushell", "version": 0, "text": text });
    lsp_notification("textDocument/didOpen", &json!({ "textDocument": document }))
}

fn lsp_formatting(id: i32, uri: &str) -> Value {
    let options = json!({ "tabSize": 8, "insertSpaces": false });
    let params = json!({ "textDocument": { "uri": uri }, "options": options });
    lsp_request(id, "textDocument/formatting", &params)
}

fn lsp_block_edits(indent: &str) -> Value {
    let range = json!({
        "start": { "line": 1, "character": 0 },
        "end": { "line": 2, "character": 0 },
    });
    json!([{ "range": range, "newText": format!("{indent}ls\n") }])
}

#[test]
fn lsp_formats_open_document_with_config_file_nearest_its_file() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "indent_width": 2 }"#);
    let uri = format!("file://{}", dir.path("sub/a.nu").display());
    let messages: Vec<_> = lsp_initialization()
        .into_iter()
        .chain([
            lsp_opening(&uri, &format!("{BLOCK}\n")),
            lsp_formatting(1, &uri),
            lsp_request(2, "shutdown", &Value::Null),
            lsp_notification("exit", &Value::Null),
        ])
        .collect();
    let output = lsp_in(&dir, &[], &messages);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    let messages = lsp_messages(&output);
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert_eq!(messages[1]["id"], 1);
    assert_eq!(messages[1]["result"], lsp_block_edits("  "));
    assert_eq!(messages[2]["id"], 2);
    assert_eq!(messages[2]["result"], Value::Null);
}

#[test]
fn lsp_takes_config_file_nearest_current_directory_for_document_without_file() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "indent_width": 2 }"#);
    let uri = "untitled:Untitled-1";
    let messages: Vec<_> = lsp_initialization()
        .into_iter()
        .chain([
            lsp_opening(uri, &format!("{BLOCK}\n")),
            lsp_formatting(1, uri),
            lsp_request(2, "shutdown", &Value::Null),
            lsp_notification("exit", &Value::Null),
        ])
        .collect();
    let output = lsp_in(&dir, &[], &messages);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let messages = lsp_messages(&output);
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert_eq!(messages[1]["result"], lsp_block_edits("  "));
}

#[test]
fn lsp_takes_stdio_flag_that_clients_pass() {
    let dir = TempDir::new();
    let messages: Vec<_> = lsp_initialization()
        .into_iter()
        .chain([
            lsp_request(1, "shutdown", &Value::Null),
            lsp_notification("exit", &Value::Null),
        ])
        .collect();
    let output = lsp_in(&dir, &["--stdio"], &messages);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lsp_messages(&output).len(), 2);
}

#[test]
fn lsp_exits_with_1_on_exit_without_shutdown() {
    let dir = TempDir::new();
    let messages: Vec<_> = lsp_initialization()
        .into_iter()
        .chain([lsp_notification("exit", &Value::Null)])
        .collect();
    let output = lsp_in(&dir, &[], &messages);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[test]
fn lsp_exits_with_1_when_stdin_ends_without_exit() {
    let dir = TempDir::new();
    let output = lsp_in(&dir, &[], &lsp_initialization());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(lsp_messages(&output).len(), 1);
}

#[test]
fn lsp_exits_with_1_when_stdin_ends_before_initialize() {
    let dir = TempDir::new();
    let output = lsp_in(&dir, &[], &[]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
