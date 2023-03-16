use std::collections::HashSet;
use std::{env, io, process};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const DEFAULT_HTTP_PROTOCOL: &str = "HTTP/1.1";

static mut A_HOST: Option<String> = None;
static mut A_PORT: Option<u16> = None;
static mut A_ALLOWED_METHODS: Option<HashSet<String>> = None;
static mut A_DISALLOWED_METHODS: Option<HashSet<String>> = None;

struct BytesFind<'a> {
    pattern: &'a [u8],
    len: usize,
    i: usize,
    count: usize,
}

impl<'a> BytesFind<'a> {
    fn new(pattern: &'a [u8]) -> Result<Self, String> {
        let len = pattern.len();
        if len == 0 {
            Err("The length of pattern is 0".to_string())
        } else {
            Ok(Self {
                pattern,
                len,
                i: 0,
                count: 0,
            })
        }
    }

    fn reset(&mut self) -> usize {
        self.i = 0;
        let count = self.count;
        self.count = 0;
        count
    }

    fn index(&self) -> Option<usize> {
        if self.i == self.len {
            Some(self.count - self.len)
        } else {
            None
        }
    }

    fn find_unsafe(&mut self, byte: &u8) -> Option<usize> {
        if *byte == self.pattern[self.i] {
            self.i += 1;
        } else if self.i != 0 {
            self.i = 0;
        }
        self.count += 1;
        self.index()
    }

    fn find(&mut self, byte: &u8) -> Option<usize> {
        match self.index() {
            None => {}
            Some(i) => {
                return Some(i);
            }
        }
        self.find_unsafe(byte)
    }

    fn finds(&mut self, bytes: &[u8]) -> Option<usize> {
        match self.index() {
            None => {}
            Some(i) => {
                return Some(i);
            }
        }
        for byte in bytes {
            match self.find_unsafe(byte) {
                None => {}
                Some(i) => {
                    return Some(i);
                }
            }
        }
        None
    }
}

#[test]
fn test_find() {
    let mut bf = BytesFind::new(b"666").unwrap();
    assert_eq!(bf.find(&b'6'), None);
    assert_eq!(bf.find(&b'6'), None);
    assert_eq!(bf.find(&b'6'), Some(0));
    assert_eq!(bf.find(&b'6'), Some(0));
    bf.reset();
    assert_eq!(bf.find(&b'0'), None);
    assert_eq!(bf.find(&b'6'), None);
    assert_eq!(bf.find(&b'6'), None);
    assert_eq!(bf.find(&b'6'), Some(1));
    assert_eq!(bf.find(&b'6'), Some(1));

    bf.reset();
    assert_eq!(bf.finds(b"666,666666"), Some(0));
    assert_eq!(bf.finds(b","), Some(0));
    bf.reset();
    assert_eq!(bf.finds(b"6,666,666666"), Some(2));
    assert_eq!(bf.finds(b"666"), Some(2));
    bf.reset();
    assert_eq!(bf.finds(b"6,6"), None);
    assert_eq!(bf.finds(b"66,666666"), Some(2));
    bf.reset();
    assert_eq!(bf.finds(b"6,6"), None);
    assert_eq!(bf.finds(b"6"), None);
    assert_eq!(bf.finds(b"6,666666"), Some(2));
}

trait TcpStreamPrintError: Write {
    fn pe_write_handle<T>(result: io::Result<T>) {
        match result {
            Err(e) => {
                eprintln!("Error writing to TcpStream: {}", e);
            }
            _ => {}
        }
    }

    fn pe_write_all(&mut self, buf: &[u8]) {
        Self::pe_write_handle(self.write_all(buf));
    }

    fn pe_write_str(&mut self, string: &str) {
        self.pe_write_all(string.as_bytes());
    }

    fn pe_write_resp_line(&mut self, protocol: &str, code: u16) {
        self.pe_write_str(protocol);
        self.pe_write_all(b" ");
        self.pe_write_all(code.to_string().as_bytes());
        self.pe_write_all(b" ");
        self.pe_write_all(match code {
            200 => b"OK",
            405 => b"Method not allowed",
            500 => b"Internal Server Error",
            _ => b"Unknown Status",
        });
        self.pe_write_all(if code == 200 {
            b"\r\n"
        } else {
            b"\r\n\r\n"
        });
    }

    fn pe_write_def_resp_line(&mut self, code: u16) {
        self.pe_write_resp_line(DEFAULT_HTTP_PROTOCOL, code);
    }

    fn pe_write_header(&mut self, name: &[u8], value: &[u8]) {
        self.pe_write_all(name);
        self.pe_write_all(b": ");
        self.pe_write_all(value);
        self.pe_write_all(b"\r\n");
    }
}

impl TcpStreamPrintError for TcpStream {}

fn parse_set(s: &str) -> Option<HashSet<String>> {
    let mut set = HashSet::new();
    for mut s in s.split(',') {
        s = s.trim();
        if s == "" {
            continue;
        }
        let s = s.to_string().to_ascii_uppercase();
        set.insert(s);
    }
    if set.len() == 0 {
        None
    } else {
        Some(set)
    }
}

fn parse_args() {
    let args: Vec<String> = env::args().collect();
    let mut option: Option<&str> = None;
    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            continue;
        }
        match option {
            None => {
                if !arg.starts_with('-') {
                    panic!("It is not a option: {}", arg);
                }
                if arg == "--help" {
                    let mut bin = args.get(0).unwrap().clone();
                    if bin.contains(' ') {
                        bin = format!("\"{}\"", bin).to_string();
                    }
                    unsafe {
                        print!("Usage: {} [options]
Options:
  -h, --host <host>
        Listen host. (default \"{}\")
  -p, --port <port>
        Listen port. If 0 is random. (default {})
  -m, --allowed-methods <method>[,<methods>...]
        Disallowed methods.
  -d, --disallowed-methods <method>[,<methods>...]
        Allowed methods.
  --help
        Print help.

Notes:
  * Cannot listen IPv4 and IPv6 at the same time on Windows.
", bin, A_HOST.as_ref().unwrap(), A_PORT.as_ref().unwrap());
                    }
                    process::exit(0);
                }
                option = Some(arg);
            }
            Some(name) => unsafe {
                option = None;
                match name {
                    "-h" | "--host"  => {
                        A_HOST = Some(arg.to_string());
                    }
                    "-p" | "--port" => {
                        A_PORT = Some(arg.parse().unwrap());
                    }
                    "-m" | "--allowed-methods" => {
                        A_ALLOWED_METHODS = parse_set(arg);
                    }
                    "-d" | "--disallowed-methods" => {
                        A_DISALLOWED_METHODS = parse_set(arg);
                    }
                    _ => {
                        panic!("Unknown option: {}", name);
                    }
                }
            }
        }
    }

    match option {
        Some(name) => {
            panic!("No found that option value: {}", name)
        }
        _ => {}
    }
}

fn handle_tcp_stream(mut stream: TcpStream) {
    let mut has_request_line = false;
    let mut line_bf = BytesFind::new(b"\r\n").unwrap();
    let mut headers_bf = BytesFind::new(b"\r\n\r\n").unwrap();
    let mut msg = Vec::with_capacity(4096);
    let mut buffer = [0; 2048];
    let mut content_length: Option<usize> = None;
    let mut content_i = 0;
    'read:
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                break;
            },
            Ok(n) => {
                'byte:
                for byte in &buffer[..n] {
                    msg.push(*byte);
                    match content_length {
                        Some(length) => {
                            content_i += 1;
                            if content_i >= length {
                                break 'read;
                            }
                            continue;
                        }
                        None => {}
                    }
                    if !has_request_line {
                        match line_bf.find(byte) {
                            None => {}
                            Some(end) => {
                                has_request_line = true;
                                let start_line = String::from_utf8_lossy(&msg[..end]);
                                println!("{}", start_line);
                                let arr: Vec<&str> = start_line.split(' ')
                                    .filter(|&s| !s.is_empty()).collect();
                                if arr.len() != 3 {
                                    eprintln!("Error HTTP request line: {}", start_line);
                                    stream.pe_write_def_resp_line(500);
                                    return;
                                }
                                let method = arr[0].to_ascii_uppercase();
                                let protocol = arr[2];
                                if unsafe {
                                    (match A_DISALLOWED_METHODS.as_ref() {
                                        None => {false}
                                        Some(set) => {set.contains(&method)}
                                    }) || (match A_ALLOWED_METHODS.as_ref() {
                                        None => {false}
                                        Some(set) => {!set.contains(&method)}
                                    })
                                } {
                                    stream.pe_write_resp_line(protocol, 405);
                                    return;
                                }
                                stream.pe_write_resp_line(protocol, 200);
                                stream.pe_write_header(
                                    b"Content-Type", b"text/plain; charset=utf-8");
                                if method == "HEAD" {
                                    return;
                                }
                            }
                        }
                    }
                    match headers_bf.find(byte) {
                        None => {}
                        Some(end) => {
                            let headers_bytes = &msg[line_bf.reset()..end + 2];
                            let mut start = 0;
                            loop {
                                match line_bf.finds(&headers_bytes[start..]) {
                                    None => {
                                        break;
                                    }
                                    Some(end) => {
                                        let header_bytes = &headers_bytes[start..end];
                                        let mut name_bf = BytesFind::new(b":")
                                            .unwrap();
                                        match name_bf.finds(header_bytes) {
                                            None => {}
                                            Some(i) => {
                                                let name = String::from_utf8_lossy(
                                                    &header_bytes[..i])
                                                    .trim().to_ascii_lowercase();
                                                if name == "content-length" {
                                                    let value = String::from_utf8_lossy(
                                                        &header_bytes[i+1..]);
                                                    let value = value.trim();
                                                    match value.parse() {
                                                        Ok(i) => {
                                                            content_length = Some(i);
                                                            continue 'byte;
                                                        }
                                                        Err(e) => {
                                                            eprintln!("Error parse str (\"{}\") \
                                                            to usize: {}", value, e);
                                                        }
                                                    }
                                                    break 'read;
                                                }
                                            }
                                        };
                                        start = line_bf.reset();
                                        line_bf.count = start;
                                    }
                                }
                            }
                            break 'read;
                        }
                    }
                };
            },
            Err(e) => {
                eprintln!("Error reading from TcpStream: {}", e);
                break 'read;
            }
        }
    }

    stream.pe_write_header(b"Content-Length", (12 + msg.len()).to_string().as_bytes());
    stream.pe_write_all(b"\r\nHello HTTP\n\n");
    stream.pe_write_all(&msg);
}

fn main() {

    unsafe {
        A_HOST = Some("127.0.0.1".to_string());
        A_PORT = Some(8080);
    }

    parse_args();

    let (host, port) = unsafe { (
        A_HOST.as_ref().unwrap(),
        A_PORT.as_ref().unwrap(),
    ) };
    let listener = TcpListener::bind(format!("{}:{}", host, port)).unwrap();
    println!("Listening {:?}", listener.local_addr().unwrap());
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(|| {
                    handle_tcp_stream(stream);
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}
