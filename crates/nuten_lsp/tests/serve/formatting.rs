//! Formatting open documents with the config of their files.

use lsp_server::ErrorCode;
use lsp_types::{
    OneOf, Position, Range, TextEdit,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as NotificationKind,
    },
    request::{Formatting, Request as RequestKind},
};
use serde_json::json;

use crate::support::{Client, TempDir, file_uri};

fn edit(start: (u32, u32), end: (u32, u32), new_text: &str) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position::new(start.0, start.1),
            end: Position::new(end.0, end.1),
        },
        new_text: new_text.to_string(),
    }
}

fn text_unknown(uri: &str) -> String {
    format!("{uri}: its text is unknown, since it is not open or was changed by a range")
}

#[test]
fn server_offers_whole_document_formatting() {
    let (client, result) = Client::initialized();
    let provider = result.capabilities.document_formatting_provider;
    assert_eq!(provider, Some(OneOf::Left(true)));
    client.shut_down();
}

#[test]
fn formatting_replaces_lines_that_change() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls\nls   |   length\nls\n");
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((1, 0), (2, 0), "ls | length\n")]));
    client.shut_down();
}

#[test]
fn formatting_formatted_document_takes_no_edits() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls\n");
    assert_eq!(client.format(&uri).unwrap(), Some(Vec::new()));
    client.shut_down();
}

#[test]
fn formatting_takes_text_client_holds_rather_than_file() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", "ls\n"));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls  \n");
    client.change(&uri, &json!([{ "text": "ls  |  length\n" }]));
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((0, 0), (1, 0), "ls | length\n")]));
    client.shut_down();
}

#[test]
fn several_whole_changes_leave_last_text() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls\n");
    client.change(
        &uri,
        &json!([{ "text": "ls  \n" }, { "text": "ls  |  length\n" }]),
    );
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((0, 0), (1, 0), "ls | length\n")]));
    client.shut_down();
}

#[test]
fn change_without_content_changes_keeps_text() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls  \n");
    client.change(&uri, &json!([]));
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((0, 0), (1, 0), "ls\n")]));
    client.shut_down();
}

#[test]
fn change_of_range_leaves_document_unknown() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls\n");
    let range = json!({
        "start": { "line": 0, "character": 2 },
        "end": { "line": 0, "character": 2 },
    });
    client.change(&uri, &json!([{ "range": range, "text": "   |   length" }]));
    let error = client.format(&uri).unwrap_err();
    assert_eq!(error.code, ErrorCode::RequestFailed as i32);
    assert_eq!(error.message, text_unknown(&uri));
    client.shut_down();
}

#[test]
fn notification_with_unreadable_params_leaves_open_documents() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls  \n");
    for method in [
        DidOpenTextDocument::METHOD,
        DidChangeTextDocument::METHOD,
        DidCloseTextDocument::METHOD,
    ] {
        client.notify(method, json!({ "textDocument": 1 }));
    }
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((0, 0), (1, 0), "ls\n")]));
    client.shut_down();
}

#[test]
fn formatting_document_with_syntax_error_answers_null() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.path("a.nu"));
    let (client, _) = Client::initialized();
    client.open(&uri, "echo   (\n");
    assert_eq!(client.format(&uri).unwrap(), None);
    client.shut_down();
}

#[test]
fn formatting_follows_config_file_nearest_file() {
    let dir = TempDir::new();
    dir.file(
        "nuten.json",
        r#"{ "indent_width": 2, "line_ending": "crlf" }"#,
    );
    let uri = file_uri(&dir.file("sub/a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "if true {\nls\n}\n");
    let edits = client.format(&uri).unwrap();
    let expected = edit((0, 0), (3, 0), "if true {\r\n  ls\r\n}\r\n");
    assert_eq!(edits, Some(vec![expected]));
    client.shut_down();
}

#[test]
fn formatting_follows_config_file_changed_after_open() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "if true {\nls\n}\n");
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((1, 0), (2, 0), "    ls\n")]));
    dir.file("nuten.json", r#"{ "indent_width": 2 }"#);
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((1, 0), (2, 0), "  ls\n")]));
    client.shut_down();
}

#[test]
fn file_not_yet_written_takes_config_of_nearest_existing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "indent_width": 2 }"#);
    let uri = file_uri(&dir.path("new/dir/a.nu"));
    let (client, _) = Client::initialized();
    client.open(&uri, "if true {\nls\n}\n");
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((1, 0), (2, 0), "  ls\n")]));
    client.shut_down();
}

#[cfg(unix)]
#[test]
fn formatting_file_exclude_matches_where_symlinked_directory_leads_answers_null() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "exclude": ["vendor"] }"#);
    dir.file("vendor/a.nu", "");
    std::os::unix::fs::symlink(dir.path("vendor"), dir.path("link")).unwrap();
    let (client, _) = Client::initialized();
    for name in ["link/a.nu", "link/new/a.nu"] {
        let uri = file_uri(&dir.path(name));
        client.open(&uri, "ls   |   length\n");
        assert_eq!(client.format(&uri).unwrap(), None, "{name}");
    }
    client.shut_down();
}

#[test]
fn formatting_file_exclude_matches_answers_null() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "exclude": ["vendor"] }"#);
    let written = file_uri(&dir.file("vendor/a.nu", ""));
    let unwritten = file_uri(&dir.path("vendor/new/a.nu"));
    let (client, _) = Client::initialized();
    for uri in [&written, &unwritten] {
        client.open(uri, "ls   |   length\n");
        assert_eq!(client.format(uri).unwrap(), None, "{uri}");
    }
    client.shut_down();
}

#[test]
fn formatting_takes_overrides_for_file_they_pick() {
    let dir = TempDir::new();
    dir.file(
        "nuten.json",
        r#"{ "overrides": [{ "include": ["d/*.nu"], "indent_width": 2 }] }"#,
    );
    let picked = file_uri(&dir.path("d/a.nu"));
    let other = file_uri(&dir.path("e/a.nu"));
    let (client, _) = Client::initialized();
    for (uri, indent) in [(&picked, "  ls\n"), (&other, "    ls\n")] {
        client.open(uri, "if true {\nls\n}\n");
        let edits = client.format(uri).unwrap();
        assert_eq!(edits, Some(vec![edit((1, 0), (2, 0), indent)]), "{uri}");
    }
    client.shut_down();
}

#[test]
fn formatting_takes_file_include_misses() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{ "include": ["*.nush"] }"#);
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls  \n");
    let edits = client.format(&uri).unwrap();
    assert_eq!(edits, Some(vec![edit((0, 0), (1, 0), "ls\n")]));
    client.shut_down();
}

#[test]
fn wrong_config_file_fails_formatting_with_its_problems() {
    let dir = TempDir::new();
    let config = dir.file("nuten.json", "{\n  \"indent_width\": 0,\n  \"x\": 1\n}\n");
    let uri = file_uri(&dir.file("a.nu", ""));
    let (client, _) = Client::initialized();
    client.open(&uri, "ls   |   length\n");
    let error = client.format(&uri).unwrap_err();
    assert_eq!(error.code, ErrorCode::RequestFailed as i32);
    let config = config.display();
    let expected = format!(
        "{config}:2:19: `indent_width` takes an integer from 1 to 255\n\
         {config}:3:3: unknown key `x`"
    );
    assert_eq!(error.message, expected);
    client.shut_down();
}

#[test]
fn document_whose_file_is_unknown_fails_formatting() {
    let (client, _) = Client::initialized();
    for uri in ["file://host/a.nu", "git:/a/b.nu"] {
        client.open(uri, "ls   |   length\n");
        let error = client.format(uri).unwrap_err();
        assert_eq!(error.code, ErrorCode::RequestFailed as i32, "{uri}");
        assert!(error.message.starts_with(&format!("{uri}: ")), "{uri}");
    }
    client.shut_down();
}

#[test]
fn document_not_open_fails_formatting() {
    let dir = TempDir::new();
    let uri = file_uri(&dir.file("a.nu", "ls   |   length\n"));
    let (client, _) = Client::initialized();
    let error = client.format(&uri).unwrap_err();
    assert_eq!(error.code, ErrorCode::RequestFailed as i32);
    assert_eq!(error.message, text_unknown(&uri));
    client.open(&uri, "ls   |   length\n");
    client.close(&uri);
    let error = client.format(&uri).unwrap_err();
    assert_eq!(error.message, text_unknown(&uri));
    client.shut_down();
}

#[test]
fn formatting_with_unreadable_params_is_invalid() {
    let (client, _) = Client::initialized();
    let error = client
        .request(Formatting::METHOD, json!({ "textDocument": 1 }))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    client.shut_down();
}
