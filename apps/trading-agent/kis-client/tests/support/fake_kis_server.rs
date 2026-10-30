use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

impl FakeResponse {
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        Self {
            status,
            headers,
            body: body.into(),
        }
    }

    #[allow(dead_code)]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }
}

#[derive(Debug)]
pub struct FakeKisServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    responses: Arc<Mutex<VecDeque<FakeResponse>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FakeKisServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fake server binds");
        listener
            .set_nonblocking(true)
            .expect("fake server nonblocking listener");
        let address = listener.local_addr().expect("fake server local address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let thread_requests = Arc::clone(&requests);
        let thread_responses = Arc::clone(&responses);
        let thread_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _peer)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .expect("fake stream read timeout");
                        let request = read_request(&mut stream);
                        thread_requests.lock().expect("request lock").push(request);
                        let response = thread_responses
                            .lock()
                            .expect("response lock")
                            .pop_front()
                            .unwrap_or_else(|| {
                                FakeResponse::json(500, r#"{"error":"no fake response queued"}"#)
                            });
                        write_response(&mut stream, &response);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("fake server accept failed: {error}"),
                }
            }
        });

        Self {
            base_url: format!("http://{address}"),
            requests,
            responses,
            shutdown,
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn push_response(&self, response: FakeResponse) {
        self.responses
            .lock()
            .expect("response lock")
            .push_back(response);
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("request lock").clone()
    }
}

impl Drop for FakeKisServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().expect("fake server thread joins");
        }
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> RecordedRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).expect("fake server reads request");
        if read == 0 {
            break find_header_end(&buffer).expect("request has headers");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let header_bytes = &buffer[..header_end];
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut lines = header_text.lines();
    let request_line = lines.next().expect("request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().expect("request method").to_owned();
    let path = request_parts.next().expect("request path").to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut chunk).expect("fake server reads body");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body_end = body_start + content_length;
    let body = String::from_utf8_lossy(&buffer[body_start..body_end]).to_string();

    RecordedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn write_response(stream: &mut std::net::TcpStream, response: &FakeResponse) {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\ncontent-length: {}\r\nconnection: close\r\n",
        response.status,
        reason,
        response.body.len()
    )
    .expect("fake server writes status");
    for (name, value) in &response.headers {
        write!(stream, "{name}: {value}\r\n").expect("fake server writes header");
    }
    write!(stream, "\r\n{}", response.body).expect("fake server writes body");
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}
