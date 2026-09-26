//! A language server for Nushell.

use std::{collections::HashMap, ops::ControlFlow};

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeResult, ServerCapabilities, ServerInfo, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit as ExitNotification,
        Notification as NotificationKind,
    },
    request::{Initialize, Request as RequestKind, Shutdown},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// With `exit` after `shutdown`, as the protocol asks.
    AfterShutdown,
    /// With `exit` alone, or by closing the connection.
    WithoutShutdown,
}

pub fn serve(connection: &Connection) -> Exit {
    if let Err(exit) = initialize(connection) {
        return exit;
    }
    let ended = connection
        .receiver
        .iter()
        .try_fold(Session::default(), |session, message| {
            session.step(message, connection)
        });
    match ended {
        ControlFlow::Break(exit) => exit,
        ControlFlow::Continue(_) => Exit::WithoutShutdown,
    }
}

/// Answers `initialize`, the first request the protocol lets the server serve.
/// The error tells how the client ended the session before it.
/// Not `Connection::initialize`, which treats an `exit` before it as a broken protocol.
fn initialize(connection: &Connection) -> Result<(), Exit> {
    connection
        .receiver
        .iter()
        .find_map(|message| match message {
            Message::Request(request) if request.method == Initialize::METHOD => Some(send(
                connection,
                Response::new_ok(request.id, initialize_result()),
            )),
            Message::Request(request) => {
                let response = Response::new_err(
                    request.id,
                    ErrorCode::ServerNotInitialized as i32,
                    "the server is not initialized".to_string(),
                );
                send(connection, response).err().map(Err)
            },
            Message::Notification(notification)
                if notification.method == ExitNotification::METHOD =>
            {
                Some(Err(Exit::WithoutShutdown))
            },
            Message::Notification(_) | Message::Response(_) => None,
        })
        .unwrap_or(Err(Exit::WithoutShutdown))
}

fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    // Not incremental, so that no range can be applied to the wrong text.
                    change: Some(TextDocumentSyncKind::FULL),
                    ..TextDocumentSyncOptions::default()
                },
            )),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "nuten".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    }
}

/// The error tells that the session ended, since nothing can reach the client any more.
fn send(connection: &Connection, response: Response) -> Result<(), Exit> {
    connection
        .sender
        .send(response.into())
        .map_err(|_| Exit::WithoutShutdown)
}

#[derive(Debug, Default)]
struct Session {
    /// The text of each open document, as the client holds it, by its URI.
    /// Not keyed by `Uri`, whose hash clippy cannot tell to be stable.
    documents: HashMap<String, String>,
    shut_down: bool,
}

#[derive(Debug)]
enum Change {
    Whole(String),
    Nothing,
    /// Makes it one the server cannot know, since the server applies no range.
    Unknown,
}

impl Session {
    fn step(self, message: Message, connection: &Connection) -> ControlFlow<Exit, Session> {
        match message {
            Message::Notification(notification)
                if notification.method == ExitNotification::METHOD =>
            {
                ControlFlow::Break(if self.shut_down {
                    Exit::AfterShutdown
                } else {
                    Exit::WithoutShutdown
                })
            },
            Message::Notification(notification) => {
                ControlFlow::Continue(self.notified(notification))
            },
            Message::Request(request) => {
                let (session, response) = self.answered(request);
                match send(connection, response) {
                    Ok(()) => ControlFlow::Continue(session),
                    Err(exit) => ControlFlow::Break(exit),
                }
            },
            Message::Response(_) => ControlFlow::Continue(self),
        }
    }

    fn answered(self, request: Request) -> (Session, Response) {
        if self.shut_down {
            (self, invalid_request(request.id, "the server is shut down"))
        } else if request.method == Initialize::METHOD {
            (
                self,
                invalid_request(request.id, "the server is already initialized"),
            )
        } else if request.method == Shutdown::METHOD {
            let session = Session {
                shut_down: true,
                ..self
            };
            (session, Response::new_ok(request.id, ()))
        } else {
            (self, method_not_found(request.id, &request.method))
        }
    }

    /// A notification whose params cannot be read is dropped, since it cannot be answered.
    fn notified(self, notification: Notification) -> Session {
        let Notification {
            method,
            params,
        } = notification;
        match method.as_str() {
            DidOpenTextDocument::METHOD => {
                match serde_json::from_value::<DidOpenTextDocumentParams>(params) {
                    Ok(params) => {
                        let document = params.text_document;
                        self.with_document(&document.uri, document.text)
                    },
                    Err(_) => self,
                }
            },
            DidChangeTextDocument::METHOD => {
                match serde_json::from_value::<DidChangeTextDocumentParams>(params) {
                    Ok(params) => {
                        let uri = &params.text_document.uri;
                        match change_of(params.content_changes) {
                            Change::Whole(text) => self.with_document(uri, text),
                            Change::Nothing => self,
                            Change::Unknown => self.without_document(uri),
                        }
                    },
                    Err(_) => self,
                }
            },
            DidCloseTextDocument::METHOD => {
                match serde_json::from_value::<DidCloseTextDocumentParams>(params) {
                    Ok(params) => self.without_document(&params.text_document.uri),
                    Err(_) => self,
                }
            },
            _ => self,
        }
    }

    fn with_document(self, uri: &Uri, text: String) -> Session {
        let mut documents = self.documents;
        documents.insert(uri.as_str().to_string(), text);
        Session {
            documents,
            ..self
        }
    }

    fn without_document(self, uri: &Uri) -> Session {
        let mut documents = self.documents;
        documents.remove(uri.as_str());
        Session {
            documents,
            ..self
        }
    }
}

fn invalid_request(id: RequestId, message: &str) -> Response {
    Response::new_err(id, ErrorCode::InvalidRequest as i32, message.to_string())
}

fn method_not_found(id: RequestId, method: &str) -> Response {
    Response::new_err(
        id,
        ErrorCode::MethodNotFound as i32,
        format!("unknown method `{method}`"),
    )
}

/// Not applying a range, which a client that syncs whole texts has no reason to send.
fn change_of(changes: Vec<TextDocumentContentChangeEvent>) -> Change {
    if changes.iter().any(|change| change.range.is_some()) {
        Change::Unknown
    } else {
        changes
            .into_iter()
            .last()
            .map_or(Change::Nothing, |change| Change::Whole(change.text))
    }
}
