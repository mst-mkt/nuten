mod edit;
mod place;

use std::{collections::HashMap, ops::ControlFlow, panic};

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, InitializeResult, OneOf, ServerCapabilities, ServerInfo,
    TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextEdit, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit as ExitNotification,
        Notification as NotificationKind,
    },
    request::{Formatting, Initialize, Request as RequestKind, Shutdown},
};
use nuten_config::Config;

use crate::place::Place;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    AfterShutdown,
    WithoutShutdown,
}

pub fn serve_stdio() -> Result<Exit, String> {
    let (connection, io_threads) = Connection::stdio();
    let exit = serve(&connection);
    // Lets the writer finish, which runs until every sender is gone.
    drop(connection);
    io_threads
        .join()
        .map_err(|error| format!("stdio: {error}"))?;
    Ok(exit)
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
            document_formatting_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "nuten".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    }
}

/// A failed send ends the session, since nothing can reach the client any more.
fn send(connection: &Connection, response: Response) -> Result<(), Exit> {
    connection
        .sender
        .send(response.into())
        .map_err(|_| Exit::WithoutShutdown)
}

#[derive(Debug, Default)]
struct Session {
    /// Not keyed by `Uri`, whose hash clippy cannot tell to be stable.
    documents: HashMap<String, String>,
    shut_down: bool,
}

#[derive(Debug)]
enum Change {
    Whole(String),
    Nothing,
    /// The server applies no range, so a change by one leaves the document unknown.
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
            let response = self.response_to(request);
            (self, response)
        }
    }

    fn response_to(&self, request: Request) -> Response {
        let Request {
            id,
            method,
            params,
        } = request;
        match method.as_str() {
            Formatting::METHOD => {
                match serde_json::from_value::<DocumentFormattingParams>(params) {
                    Ok(params) => match self.formatting(&params.text_document.uri) {
                        Ok(edits) => Response::new_ok(id, edits),
                        Err((code, message)) => Response::new_err(id, code as i32, message),
                    },
                    Err(error) => {
                        Response::new_err(id, ErrorCode::InvalidParams as i32, error.to_string())
                    },
                }
            },
            _ => method_not_found(id, &method),
        }
    }

    fn formatting(&self, uri: &Uri) -> Result<Option<Vec<TextEdit>>, (ErrorCode, String)> {
        let uri_text = uri.as_str();
        let text = self.documents.get(uri_text).ok_or_else(|| {
            let problem = "its text is unknown, since it is not open or was changed by a range";
            (ErrorCode::RequestFailed, format!("{uri_text}: {problem}"))
        })?;
        let place = place::of(uri);
        // The session goes on, since the other documents may still be formatted.
        panic::catch_unwind(|| formatting_at(&place, uri_text, text)).unwrap_or_else(|_| {
            Err((
                ErrorCode::InternalError,
                format!("{uri_text}: nuten panicked while formatting"),
            ))
        })
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

fn formatting_at(
    place: &Place,
    uri: &str,
    text: &str,
) -> Result<Option<Vec<TextEdit>>, (ErrorCode, String)> {
    let failed = |problems: Vec<String>| (ErrorCode::RequestFailed, problems.join("\n"));
    // Found again for each request, so that a config file edited meanwhile is never missed.
    let config = config_of(place, uri).map_err(failed)?;
    let file = match place {
        Place::File(path) => Some(path.as_path()),
        _ => None,
    };
    let Some(format) = config
        .format_of_text(file)
        .map_err(|problem| failed(vec![problem]))?
    else {
        return Ok(None);
    };
    Ok(match nuten_format::format(text, &format) {
        Ok(formatted) => Some(edit::between(text, &formatted).into_iter().collect()),
        // Not an error, which the client would show for code that is still being typed.
        Err(_) => None,
    })
}

fn config_of(place: &Place, uri: &str) -> Result<Config, Vec<String>> {
    match place {
        Place::File(path) => nuten_config::of_text(Some(path)),
        Place::NoFile => nuten_config::of_text(None),
        // Not the config of another place, which could override the author's.
        Place::UnknownFile => Err(vec![format!(
            "{uri}: its file, and so its config, is unknown"
        )]),
    }
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
