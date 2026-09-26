//! Starting a session, answering what the server does not serve, and ending the session.

use lsp_server::ErrorCode;
use lsp_types::{
    TextDocumentSyncCapability, TextDocumentSyncKind,
    request::{Initialize, Request as RequestKind, Shutdown},
};
use nuten_lsp::Exit;
use serde_json::{Value, json};

use crate::support::Client;

#[test]
fn server_asks_for_whole_text_of_each_open_document() {
    let (client, result) = Client::initialized();
    let Some(TextDocumentSyncCapability::Options(sync)) = result.capabilities.text_document_sync
    else {
        panic!("{:?}", result.capabilities.text_document_sync);
    };
    assert_eq!(sync.open_close, Some(true));
    assert_eq!(sync.change, Some(TextDocumentSyncKind::FULL));
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn server_names_itself_nuten() {
    let (client, result) = Client::initialized();
    assert_eq!(result.server_info.unwrap().name, "nuten");
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn request_before_initialize_is_answered_with_server_not_initialized() {
    let client = Client::start();
    let error = client.request(Shutdown::METHOD, Value::Null).unwrap_err();
    assert_eq!(error.code, ErrorCode::ServerNotInitialized as i32);
    assert_eq!(client.exit(), Exit::WithoutShutdown);
}

#[test]
fn exit_before_initialize_ends_session_without_shutdown() {
    let client = Client::start();
    client.notify("nuten/unknown", json!({}));
    assert_eq!(client.exit(), Exit::WithoutShutdown);
}

#[test]
fn closed_connection_before_initialize_ends_session_without_shutdown() {
    let client = Client::start();
    assert_eq!(client.disconnect(), Exit::WithoutShutdown);
}

#[test]
fn exit_after_shutdown_ends_session_as_protocol_asks() {
    let (client, _) = Client::initialized();
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn exit_alone_ends_session_without_shutdown() {
    let (client, _) = Client::initialized();
    assert_eq!(client.exit(), Exit::WithoutShutdown);
}

#[test]
fn closed_connection_ends_session_without_shutdown() {
    let (client, _) = Client::initialized();
    assert_eq!(client.disconnect(), Exit::WithoutShutdown);
}

#[test]
fn unknown_request_is_answered_with_method_not_found() {
    let (client, _) = Client::initialized();
    let error = client.request("nuten/unknown", Value::Null).unwrap_err();
    assert_eq!(error.code, ErrorCode::MethodNotFound as i32);
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn unknown_notification_leaves_session_going() {
    let (client, _) = Client::initialized();
    client.notify("nuten/unknown", json!({}));
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn response_from_client_leaves_session_going() {
    let (client, _) = Client::initialized();
    client.respond(0);
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn second_initialize_is_invalid() {
    let (client, _) = Client::initialized();
    let params = json!({ "capabilities": {} });
    let error = client.request(Initialize::METHOD, params).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest as i32);
    assert_eq!(client.shut_down(), Exit::AfterShutdown);
}

#[test]
fn request_after_shutdown_is_invalid() {
    let (client, _) = Client::initialized();
    let result = client.request(Shutdown::METHOD, Value::Null).unwrap();
    assert_eq!(result, Value::Null);
    for method in [Shutdown::METHOD, "nuten/unknown"] {
        let error = client.request(method, Value::Null).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest as i32, "{method}");
    }
    assert_eq!(client.exit(), Exit::AfterShutdown);
}
