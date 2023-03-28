use std::collections::HashSet;
use std::{env, io, process};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const DEFAULT_HTTP_PROTOCOL: &str = "HTTP/1.1";

static mut A_HOST: Option<String> = None;
static mut A_PORT: Option<u16> = None;
static mut A_ALLOWED_METHODS: Option<HashSet<String>> = None;
static mut A_DISALLOWED_METHODS: Option<HashSet<String>> = None;

struct SearchBytes<'a> {
    bytes: &'a [u8],
    length: usize,
    index: usize,
    count: usize,
}

impl<'a> SearchBytes<'a> {
    fn new(pattern: &'a [u8]) -> Result<Self, String> {
        let len = pattern.len();
        if len == 0 {
            Err("The length of pattern is 0".to_string())
        } else {
            Ok(Self {
                bytes: pattern,
                length: len,
                index: 0,
                count: 0,
            })
        }
    }

    fn reset(&mut self) -> usize {
        self.index = 0;
        let count = self.count;
        self.count = 0;
        count
    }

    fn result(&self) -> Option<usize> {
        if self.index == self.length {
            Some(self.count - self.length)
        } else {
            None
        }
    }

    fn _search(&mut self, byte: &u8) -> Option<usize> {
        if *byte == self.bytes[self.index] {
            self.index += 1;
        } else if self.index != 0 {
            self.index = 0;
        }
        self.count += 1;
        self.result()
    }

    fn search(&mut self, byte: &u8) -> Option<usize> {
        match self.result() {
            None => {}
            Some(i) => {
                return Some(i);
            }
        }
        self._search(byte)
    }

    fn search2(&mut self, bytes: &[u8]) -> Option<usize> {
        match self.result() {
            None => {}
            Some(i) => {
                return Some(i);
            }
        }
        for byte in bytes {
            match self._search(byte) {
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
fn test_search_bytes() {
    let mut bf = SearchBytes::new(b"666").unwrap();
    assert_eq!(bf.search(&b'6'), None);
    assert_eq!(bf.search(&b'6'), None);
    assert_eq!(bf.search(&b'6'), Some(0));
    assert_eq!(bf.search(&b'6'), Some(0));
    bf.reset();
    assert_eq!(bf.search(&b'0'), None);
    assert_eq!(bf.search(&b'6'), None);
    assert_eq!(bf.search(&b'6'), None);
    assert_eq!(bf.search(&b'6'), Some(1));
    assert_eq!(bf.search(&b'6'), Some(1));

    bf.reset();
    assert_eq!(bf.search2(b"666,666666"), Some(0));
    assert_eq!(bf.search2(b","), Some(0));
    bf.reset();
    assert_eq!(bf.search2(b"6,666,666666"), Some(2));
    assert_eq!(bf.search2(b"666"), Some(2));
    bf.reset();
    assert_eq!(bf.search2(b"6,6"), None);
    assert_eq!(bf.search2(b"66,666666"), Some(2));
    bf.reset();
    assert_eq!(bf.search2(b"6,6"), None);
    assert_eq!(bf.search2(b"6"), None);
    assert_eq!(bf.search2(b"6,666666"), Some(2));
}

trait PrintErrTcpStream: Write {
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

impl PrintErrTcpStream for TcpStream {}

fn parse_methods(s: &str) -> Option<HashSet<String>> {
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

fn print_help(args: &Vec<String>) {
    let mut file = args.get(0).unwrap().clone();
    if file.contains(' ') {
        file = format!("\"{}\"", file);
    }
    unsafe {
        print!("Usage: {} [options]

Options:
  -h <host>
        Listen host.
        (default \"{}\")
  -p <port>
        Listen port.
        If 0 is random.
        (default {})
  -m <method>[,<method>...]
        Disallowed methods.
  -d <method>[,<method>...]
        Allowed methods.
  --help
        Print help.
", file, A_HOST.as_ref().unwrap(), A_PORT.as_ref().unwrap());
    }
}

fn parse_args() {
    let args: Vec<String> = env::args().collect();
    let mut iter = args[1..].iter();
    unsafe {
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" => {
                    print_help(&args);
                    process::exit(0);
                },
                "-h" => {
                    A_HOST = Some(iter.next().unwrap().clone());
                }
                "-p" => {
                    A_PORT = Some(iter.next().unwrap().parse().unwrap());
                }
                "-m" => {
                    A_ALLOWED_METHODS = parse_methods(iter.next().unwrap());
                }
                "-d" => {
                    A_DISALLOWED_METHODS = parse_methods(iter.next().unwrap());
                }
                arg => {
                    println!("Unknown arg: {}", arg);
                    print_help(&args);
                    process::exit(1);
                }
            }
        }
    }
}

fn handle_tcp_stream(mut stream: TcpStream) {
    let mut has_request_line = false;
    let mut line_bf = SearchBytes::new(b"\r\n").unwrap();
    let mut headers_bf = SearchBytes::new(b"\r\n\r\n").unwrap();
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
                        match line_bf.search(byte) {
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
                    match headers_bf.search(byte) {
                        None => {}
                        Some(end) => {
                            let headers_bytes = &msg[line_bf.reset()..end + 2];
                            let mut start = 0;
                            loop {
                                match line_bf.search2(&headers_bytes[start..]) {
                                    None => {
                                        break;
                                    }
                                    Some(end) => {
                                        let header_bytes = &headers_bytes[start..end];
                                        let mut name_bf = SearchBytes::new(b":")
                                            .unwrap();
                                        match name_bf.search2(header_bytes) {
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
