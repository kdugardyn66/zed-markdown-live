//! A small HTTP/1.1 server: one thread per connection, keep-alive, Range-friendly
//! streaming. (tiny_http's thread pool can leave a new keep-alive connection
//! queued behind idle ones, which made Chromium wait forever for a script.)

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    stream: TcpStream,
    close: bool,
}

pub enum Body {
    Bytes(Vec<u8>),
    Reader(Box<dyn Read + Send>, u64),
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Response {
    pub fn bytes(status: u16, data: impl Into<Vec<u8>>) -> Response {
        Response { status, headers: vec![], body: Body::Bytes(data.into()) }
    }
    pub fn text(status: u16, s: &str) -> Response {
        Response::bytes(status, s.as_bytes().to_vec()).header("Content-Type", "text/plain; charset=utf-8")
    }
    pub fn reader(status: u16, r: Box<dyn Read + Send>, len: u64) -> Response {
        Response { status, headers: vec![], body: Body::Reader(r, len) }
    }
    pub fn header(mut self, k: &str, v: &str) -> Response {
        self.headers.push((k.to_string(), v.to_string()));
        self
    }
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        416 => "Range Not Satisfiable",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "",
    }
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// Writes the response. Errors (client went away) are returned, not fatal.
    pub fn respond(mut self, r: Response) -> std::io::Result<()> {
        let mut head = format!("HTTP/1.1 {} {}\r\n", r.status, reason(r.status));
        let len = match &r.body {
            Body::Bytes(b) => b.len() as u64,
            Body::Reader(_, n) => *n,
        };
        for (k, v) in &r.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str(&format!("Content-Length: {len}\r\n"));
        head.push_str(if self.close { "Connection: close\r\n" } else { "Connection: keep-alive\r\n" });
        head.push_str("\r\n");
        let _ = self.stream.set_nodelay(true);
        let mut w = std::io::BufWriter::with_capacity(64 * 1024, &self.stream);
        w.write_all(head.as_bytes())?;
        if self.method != "HEAD" {
            match r.body {
                Body::Bytes(b) => w.write_all(&b)?,
                Body::Reader(mut rd, n) => {
                    std::io::copy(&mut (&mut rd).take(n), &mut w)?;
                }
            }
        }
        w.flush()?;
        drop(w);
        self.stream.flush()
    }
}

/// Binds `127.0.0.1:port` (falls back to a random port) and serves `handler` on threads.
pub fn serve<F>(port: u16, handler: F) -> std::io::Result<u16>
where
    F: Fn(Request) + Send + Sync + 'static,
{
    let listener = TcpListener::bind(("127.0.0.1", port)).or_else(|e| if port != 0 { TcpListener::bind(("127.0.0.1", 0)) } else { Err(e) })?;
    let port = listener.local_addr()?.port();
    let handler = std::sync::Arc::new(handler);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(stream) = conn else { continue };
            let h = handler.clone();
            std::thread::spawn(move || connection(stream, &*h));
        }
    });
    Ok(port)
}

fn connection(stream: TcpStream, handler: &(dyn Fn(Request) + Send + Sync)) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let Ok(read_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read_half);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let url = parts.next().unwrap_or("/").to_string();
        let version = parts.next().unwrap_or("HTTP/1.0").to_string();
        let mut headers = Vec::new();
        loop {
            let mut h = String::new();
            match reader.read_line(&mut h) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
            if headers.len() > 100 {
                return;
            }
        }
        let get = |n: &str| headers.iter().find(|(k, _): &&(String, String)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.clone());
        let len: usize = get("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
        if len > 1024 * 1024 * 1024 {
            return;
        }
        let mut body = vec![0u8; len];
        if len > 0 && reader.read_exact(&mut body).is_err() {
            return;
        }
        let conn_hdr = get("Connection").unwrap_or_default().to_ascii_lowercase();
        let close = conn_hdr.contains("close") || (version == "HTTP/1.0" && !conn_hdr.contains("keep-alive"));
        let Ok(s) = stream.try_clone() else { return };
        handler(Request { method, url, headers, body, stream: s, close });
        if close {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
    }
}
