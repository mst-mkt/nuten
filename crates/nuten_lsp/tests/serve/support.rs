use std::{
    cell::Cell,
    env, fmt, fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response, ResponseError};
use lsp_types::{
    InitializeParams, InitializeResult, TextEdit,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit as ExitNotification,
        Initialized, Notification as NotificationKind,
    },
    request::{Formatting, Initialize, Request as RequestKind, Shutdown},
};
use nuten_lsp::Exit;
use serde_json::{Value, json};

/// Long enough for any answer, so that a server that never answers fails the test.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Its path has no symlinks, since the paths of config files are reported with symlinks resolved.
#[derive(Debug)]
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("nuten-lsp-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dir = TempDir(fs::canonicalize(&dir).unwrap());
        let above = nuten_config::find(&dir.0).unwrap();
        assert_eq!(above, None, "a config file above the temporary directory");
        let affecting: Vec<_> = dir
            .0
            .ancestors()
            .map(|ancestor| ancestor.join(".editorconfig"))
            .filter(|path| path.exists())
            .collect();
        assert!(
            affecting.is_empty(),
            "{affecting:?} above the temporary directory"
        );
        dir
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub(crate) fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// `path` must have no character a URI would escape.
#[cfg(unix)]
pub(crate) fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// `path` must have no character a URI would escape.
#[cfg(windows)]
pub(crate) fn file_uri(path: &Path) -> String {
    let path = path.display().to_string();
    format!(
        "file:///{}",
        path.trim_start_matches(r"\\?\").replace('\\', "/")
    )
}

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

    pub(crate) fn respond(&self, id: i32) {
        let response = Response::new_ok(RequestId::from(id), Value::Null);
        self.connection.sender.send(response.into()).unwrap();
    }

    pub(crate) fn open(&self, uri: &str, text: &str) {
        let document = json!({ "uri": uri, "languageId": "nushell", "version": 0, "text": text });
        self.notify(
            DidOpenTextDocument::METHOD,
            json!({ "textDocument": document }),
        );
    }

    pub(crate) fn change(&self, uri: &str, changes: &Value) {
        let params = json!({
            "textDocument": { "uri": uri, "version": 1 },
            "contentChanges": changes,
        });
        self.notify(DidChangeTextDocument::METHOD, params);
    }

    pub(crate) fn close(&self, uri: &str) {
        let params = json!({ "textDocument": { "uri": uri } });
        self.notify(DidCloseTextDocument::METHOD, params);
    }

    pub(crate) fn format(&self, uri: &str) -> Result<Option<Vec<TextEdit>>, ResponseError> {
        let params = json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 8, "insertSpaces": false },
        });
        self.request(Formatting::METHOD, params)
            .map(|result| serde_json::from_value(result).unwrap())
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
