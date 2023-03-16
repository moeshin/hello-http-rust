use std::collections::HashSet;
use std::{env, io, process};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

static mut A_HOST: Option<String> = None;
static mut A_PORT: Option<u16> = None;
static mut A_ALLOWED_METHODS: Option<HashSet<String>> = None;
static mut A_DISALLOWED_METHODS: Option<HashSet<String>> = None;

struct BytesFind<'a> {
    pattern: &'a [u8],
    len: usize,
    i: usize,
    pos: usize,
    result: Option<usize>,
}

impl<'a> BytesFind<'a> {
    fn new(pattern: &'a [u8]) -> Self {
        return Self {
            pattern,
            len: pattern.len(),
            i: 0,
            pos: 0,
            result: None,
        }
    }

    fn reset(&mut self) {
        self.i = 0;
        self.pos = 0;
        self.result = None;
    }

    fn find(&mut self, byte: u8) -> Option<usize> {
        if self.result.is_some() {
            return self.result;
        }
        if byte == self.pattern[self.i] {
            self.i += 1;
        } else if self.i != 0 {
            self.i = 0;
        }
        self.pos += 1;
        if self.i == self.len {
            self.result = Some(self.pos - self.len);
            self.result
        } else {
            None
        }
    }

    fn finds(&mut self, bytes: &[u8]) -> Option<usize> {
        if self.result.is_some() {
            return self.result;
        }
        for byte in bytes {
            if *byte == self.pattern[self.i] {
                self.i += 1;
            } else if self.i != 0 {
                self.i = 0;
            }
            self.pos += 1;
            if self.i == self.len {
                self.result = Some(self.pos - self.len);
                return self.result;
            }
        }
        None
    }
}

#[test]
fn test_find() {
    let mut bf = BytesFind::new(b"666");
    assert_eq!(bf.find(b'6'), None);
    assert_eq!(bf.find(b'6'), None);
    assert_eq!(bf.find(b'6'), Some(0));
    assert_eq!(bf.find(b'6'), Some(0));
    bf.reset();
    assert_eq!(bf.find(b'0'), None);
    assert_eq!(bf.find(b'6'), None);
    assert_eq!(bf.find(b'6'), None);
    assert_eq!(bf.find(b'6'), Some(1));
    assert_eq!(bf.find(b'6'), Some(1));
}

#[test]
fn test_finds() {
    let mut bf = BytesFind::new(b"666");
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

fn he_write(r: io::Result<usize>) {
    match r {
        Err(e) => {
            eprintln!("Error writing to TcpStream: {}", e)
        }
        _ => {}
    }
}

fn he_flush(stream: &mut TcpStream) {
    match stream.flush() {
        Err(e) => {
            eprintln!("Error flush TcpStream: {}", e);
        }
        _ => {}
    }
}

fn handle_tcp_stream(mut stream: TcpStream) {
    let mut has_request_line = false;
    let mut line_bf = BytesFind::new(b"\r\n");
    let mut headers_bf = BytesFind::new(b"\r\n\r\n");
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
                        match line_bf.find(*byte) {
                            None => {}
                            Some(end) => {
                                has_request_line = true;
                                let start_line = String::from_utf8_lossy(&msg[..end]);
                                println!("{}", start_line);
                                let arr: Vec<&str> = start_line.split(' ')
                                    .filter(|&s| !s.is_empty()).collect();
                                if arr.len() != 3 {
                                    eprintln!("Error HTTP request line: {}", start_line);
                                    he_write(stream.write(
                                        b"HTTP/1.1 500 Internal Server Error\r\n\r\n"));
                                    he_flush(&mut stream);
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
                                    println!("disallowed");
                                    he_write(stream.write(protocol.as_bytes()));
                                    he_write(stream.write(b" \
                                    405 Method not allowed\r\n\
                                    \r\n"));
                                    he_flush(&mut stream);
                                    return;
                                }
                                he_write(stream.write(protocol.as_bytes()));
                                he_write(stream.write(b" \
                                200 OK\r\n\
                                Content-Type: text/plain; charset=utf-8\r\n\
                                "));
                                if method == "HEAD" {
                                    he_flush(&mut stream);
                                    return;
                                }
                            }
                        }
                    }
                    match headers_bf.find(*byte) {
                        None => {}
                        Some(end) => {
                            let headers_bytes = &msg[line_bf.pos..end + 2];
                            line_bf.reset();
                            let mut start = 0;
                            loop {
                                match line_bf.finds(&headers_bytes[start..]) {
                                    None => {
                                        break;
                                    }
                                    Some(end) => {
                                        let header_bytes = &headers_bytes[start..end];
                                        let mut name_bf = BytesFind::new(b":");
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
                                        start = line_bf.pos;
                                        line_bf.reset();
                                        line_bf.pos = start;
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

    he_write(stream.write(b"Content-Length: "));
    he_write(stream.write((12 + msg.len()).to_string().as_bytes()));
    he_write(stream.write(b"\r\n\r\nHello HTTP\n\n"));
    he_write(stream.write(&msg));

    he_flush(&mut stream);
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
