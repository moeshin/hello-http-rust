use std::collections::HashSet;
use std::{env, io, process};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

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

    fn reset(&mut self) {
        self.index = 0;
        self.count = 0;
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

    fn pe_write_new_line(&mut self) {
        self.pe_write_all(b"\r\n");
    }

    fn pe_write_resp_line(&mut self, code: u16) {
        self.pe_write_all(b"HTTP/1.0");
        self.pe_write_all(b" ");
        self.pe_write_all(code.to_string().as_bytes());
        self.pe_write_all(b" ");
        self.pe_write_all(match code {
            200 => b"OK",
            405 => b"Method not allowed",
            500 => b"Internal Server Error",
            _ => b"Unknown Status",
        });
        self.pe_write_new_line();
        if code != 200 {
            self.pe_write_new_line();
        }
    }

    fn pe_write_header(&mut self, name: &[u8], value: &[u8]) {
        self.pe_write_all(name);
        self.pe_write_all(b": ");
        self.pe_write_all(value);
        self.pe_write_new_line();
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
    let mut line_sb = SearchBytes::new(b"\r\n").unwrap();
    let mut header_sb = SearchBytes::new(b":").unwrap();
    let mut eof = false;
    let mut request_line_ok = false;
    let mut cache = Vec::with_capacity(4096);
    let mut buf = [0; 1];
    let mut content_length = 0;
    while !eof && line_sb.result() != Some(0) {
        line_sb.reset();
        header_sb.reset();

        loop {
            match stream.read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        eof = true;
                        break;
                    }
                    let byte = &buf[0];
                    cache.push(*byte);

                    if line_sb.search(byte).is_some() {
                        break;
                    }

                    if request_line_ok {
                        header_sb.search(byte);
                    }
                },
                Err(e) => {
                    eprintln!("Error reading from TcpStream: {}", e);
                    return;
                }
            }
        }

        let line_end = cache.len() - line_sb.length;
        let line_start = line_end - line_sb.result().unwrap_or(0);
        if !request_line_ok {
            request_line_ok = true;
            let request_line = String::from_utf8_lossy(&cache[line_start..line_end]);
            println!("{}", request_line);
            let method = match request_line.find(' ') {
                None => "".to_string(),
                Some(mut i) => {
                    i += 1;
                    if i <= request_line.len() {
                        (&request_line[..i]).to_ascii_uppercase()
                    } else {
                        "".to_string()
                    }
                }
            };

            if unsafe {
                (match A_DISALLOWED_METHODS.as_ref() {
                    None => { false }
                    Some(set) => { set.contains(&method) }
                }) || (match A_ALLOWED_METHODS.as_ref() {
                    None => { false }
                    Some(set) => { !set.contains(&method) }
                })
            } {
                stream.pe_write_resp_line(405);
                return;
            }
            stream.pe_write_resp_line(200);
            stream.pe_write_header(b"Content-Type", b"text/plain; charset=utf-8");
            if method == "HEAD" {
                stream.pe_write_all(b"\r\n");
                return;
            }
            continue;
        }

        match header_sb.result() {
            None => {
                // Bad request header line
                continue;
            }
            Some(mut i) => {
                i += line_start;
                let name = String::from_utf8_lossy(&cache[line_start..i]).trim()
                    .to_ascii_lowercase();
                if name != "content-length" {
                    continue;
                }
                let value = String::from_utf8_lossy(&cache[i+header_sb.length..line_end]).trim()
                    .to_owned();
                // println!("H '{}': '{}'", name, value);
                content_length = match value.parse() {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Error parsing Content-Length str (\"{}\") to int: {}", value, e);
                        0
                    }
                }
            }
        }
    }


    while !eof && line_sb.result() != Some(0) {
        line_sb.reset();
        loop {
            match stream.read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        eof = true;
                        break;
                    }
                    let byte = &buf[0];
                    cache.push(*byte);

                    if line_sb.search(byte).is_some() {
                        break;
                    }
                },
                Err(e) => {
                    eprintln!("Error reading from TcpStream: {}", e);
                    return;
                }
            }
        }
    }

    stream.pe_write_header(b"Content-Length",
                           (12 + cache.len() + content_length).to_string().as_bytes());
    stream.pe_write_all(b"\r\nHello HTTP\n\n");
    stream.pe_write_all(&cache);
    drop(cache);

    const BUFFER_SIZE: usize = 1024;
    let mut buffer = [0; BUFFER_SIZE];
    let mut total = 0;
    while total < content_length {
        match stream.read(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                stream.pe_write_all(&buffer[..size]);
                total += size;
            }
            Err(e) => {
                eprintln!("Error coping TcpStream itself: {}", e);
            }
        }
    }
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
