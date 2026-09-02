use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// One answer the fake server is prepared to give
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Answer {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Answer {
    /// Returns a successful answer carrying `body` as JSON
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: body.into().into_bytes(),
        }
    }

    /// Returns a successful answer carrying `body` as bytes
    pub fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "application/octet-stream",
            body,
        }
    }

    /// Returns a successful answer carrying `body` as plain text
    pub fn text(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/plain",
            body: body.into().into_bytes(),
        }
    }

    /// Returns a failing answer with no body worth reading
    pub fn failure(status: u16) -> Self {
        Self {
            status,
            content_type: "text/plain",
            body: Vec::new(),
        }
    }
}

/// What one request carried, as the server saw it
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Seen {
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl Seen {
    /// Returns the value of `name`, whatever case the sender used
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// A release API that answers from a table, and records what it was asked
///
/// The tests point whisker at this instead of at GitHub. It is a real
/// server that speaks real HTTP, so the tests cover the client whisker
/// ships rather than a stand-in.
///
/// A caller adds routes after the server listens. An answer carries the
/// addresses of the assets it offers, and nobody knows those until the
/// server has a port.
pub struct FakeGitHub {
    server: Arc<tiny_http::Server>,
    url: String,
    routes: Arc<Mutex<HashMap<String, Answer>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
    thread: Option<JoinHandle<()>>,
}

impl FakeGitHub {
    /// Starts a server on a port the operating system chooses
    ///
    /// # Panics
    ///
    /// Panics if no port can be bound.
    pub fn start() -> Self {
        let server = Arc::new(
            tiny_http::Server::http("127.0.0.1:0").expect("the fake server should bind a port"),
        );
        let url = format!(
            "http://{}",
            server
                .server_addr()
                .to_ip()
                .expect("the fake server should listen on a socket")
        );

        let routes: Arc<Mutex<HashMap<String, Answer>>> = Arc::new(Mutex::new(HashMap::new()));
        let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));

        let thread = std::thread::spawn({
            let server = Arc::clone(&server);
            let routes = Arc::clone(&routes);
            let seen = Arc::clone(&seen);

            move || {
                for request in server.incoming_requests() {
                    let path = request
                        .url()
                        .split('?')
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                    let headers = request
                        .headers()
                        .iter()
                        .map(|header| {
                            (
                                header.field.as_str().as_str().to_owned(),
                                header.value.as_str().to_owned(),
                            )
                        })
                        .collect();

                    seen.lock()
                        .expect("the record should be available")
                        .push(Seen {
                            path: path.clone(),
                            headers,
                        });

                    let answer = routes
                        .lock()
                        .expect("the routes should be available")
                        .get(&path)
                        .cloned()
                        .unwrap_or_else(|| Answer::failure(404));

                    let header = tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        answer.content_type.as_bytes(),
                    )
                    .expect("the content type should be a header");

                    let _ = request.respond(
                        tiny_http::Response::from_data(answer.body)
                            .with_status_code(answer.status)
                            .with_header(header),
                    );
                }
            }
        });

        Self {
            server,
            url,
            routes,
            seen,
            thread: Some(thread),
        }
    }

    /// Returns the base address to point whisker at
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Answers `path` with `answer` from now on
    pub fn route(&self, path: &str, answer: Answer) {
        self.routes
            .lock()
            .expect("the routes should be available")
            .insert(path.to_owned(), answer);
    }

    /// Returns every request the server has seen, in order
    pub fn seen(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .expect("the record should be available")
            .clone()
    }
}

impl Drop for FakeGitHub {
    fn drop(&mut self) {
        self.server.unblock();

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
