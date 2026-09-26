//! A client that talks to the server over a connection in memory.

use std::{
    cell::Cell,
    fmt,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response, ResponseError};
use lsp_types::{
    InitializeParams, InitializeResult,
    notification::{Exit as ExitNotification, Initialized, Notification as NotificationKind},
    request::{Initialize, Request as RequestKind, Shutdown},
};
use nuten_lsp::Exit;
use serde_json::{Value, json};

/// Long enough for any answer, so that a server that never answers fails the test.
const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Client {
    connection: Connection,
    ended: Receiver<Exit>,
    next_id: Cell<i32>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client that has not yet sent `initialize`.
    pub(crate) fn start() -> Client {
        let (connection, server_connection) = Connection::memory();
        let (sender, ended) = mpsc::channel();
        thread::spawn(move || sender.send(nuten_lsp::serve(&server_connection)));
        Client {
            connection,
            ended,
            next_id: Cell::new(0),
        }
    }

    /// A client past `initialize` and `initialized`, with what the server answered.
    pub(crate) fn initialized() -> (Client, InitializeResult) {
        let client = Client::start();
        let params = serde_json::to_value(InitializeParams::default()).unwrap();
        let result = client.request(Initialize::METHOD, params).unwrap();
        client.notify(Initialized::METHOD, json!({}));
        (client, serde_json::from_value(result).unwrap())
    }

    pub(crate) fn request(&self, method: &str, params: Value) -> Result<Value, ResponseError> {
        let id = RequestId::from(self.next_id.replace(self.next_id.get() + 1));
        let request = Request::new(id.clone(), method.to_string(), params);
        self.connection.sender.send(request.into()).unwrap();
        match self.connection.receiver.recv_timeout(TIMEOUT).unwrap() {
            Message::Response(response) => {
                assert_eq!(response.id, id);
                response.response_result
            },
            message => panic!("{message:?} instead of a response"),
        }
    }

    pub(crate) fn notify(&self, method: &str, params: Value) {
        let notification = Notification::new(method.to_string(), params);
        self.connection.sender.send(notification.into()).unwrap();
    }

    /// Answers a request the server never sent.
    pub(crate) fn respond(&self, id: i32) {
        let response = Response::new_ok(RequestId::from(id), Value::Null);
        self.connection.sender.send(response.into()).unwrap();
    }

    pub(crate) fn shut_down(self) -> Exit {
        let result = self.request(Shutdown::METHOD, Value::Null).unwrap();
        assert_eq!(result, Value::Null);
        self.exit()
    }

    pub(crate) fn exit(self) -> Exit {
        self.notify(ExitNotification::METHOD, Value::Null);
        self.ended.recv_timeout(TIMEOUT).unwrap()
    }

    pub(crate) fn disconnect(self) -> Exit {
        let Client {
            connection,
            ended,
            next_id: _,
        } = self;
        drop(connection);
        ended.recv_timeout(TIMEOUT).unwrap()
    }
}
