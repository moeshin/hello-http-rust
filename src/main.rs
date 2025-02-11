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
    let mut sb = SearchBytes::new(b"666").unwrap();
    assert_eq!(sb.search(&b'6'), None);
    assert_eq!(sb.search(&b'6'), None);
    assert_eq!(sb.search(&b'6'), Some(0));
    assert_eq!(sb.search(&b'6'), Some(0));
    sb.reset();
    assert_eq!(sb.search(&b'0'), None);
    assert_eq!(sb.search(&b'6'), None);
    assert_eq!(sb.search(&b'6'), None);
    assert_eq!(sb.search(&b'6'), Some(1));
    assert_eq!(sb.search(&b'6'), Some(1));
}

trait TcpStreamExtensions: Write {
    fn write_new_line(&mut self) -> io::Result<()> {
        self.write_all(b"\r\n")
    }

    fn write_resp_line(&mut self, code: u16) -> io::Result<()> {
        self.write_all(b"HTTP/1.0")?;
        self.write_all(b" ")?;
        self.write_all(code.to_string().as_bytes())?;
        self.write_all(b" ")?;
        self.write_all(match code {
            200 => b"OK",
            405 => b"Method Not Allowed",
            500 => b"Internal Server Error",
            _ => b"Unknown Status",
        })?;
        self.write_new_line()?;
        if code != 200 {
            self.write_new_line()?;
        }
        Ok(())
    }

    fn write_header(&mut self, name: &[u8], value: &[u8]) -> io::Result<()> {
        self.write_all(name)?;
        self.write_all(b": ")?;
        self.write_all(value)?;
        self.write_new_line()
    }
}

impl TcpStreamExtensions for TcpStream {}

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
    #[allow(static_mut_refs)]
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

fn handle_tcp_stream(mut stream: TcpStream) -> io::Result<()> {
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
            if stream.read(&mut buf)? == 0 {
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

            #[allow(static_mut_refs)]
            if unsafe {
                (match A_DISALLOWED_METHODS.as_ref() {
                    None => { false }
                    Some(set) => { set.contains(&method) }
                }) || (match A_ALLOWED_METHODS.as_ref() {
                    None => { false }
                    Some(set) => { !set.contains(&method) }
                })
            } {
                return stream.write_resp_line(405);
            }
            stream.write_resp_line(200)?;
            stream.write_header(b"Content-Type", b"text/plain; charset=utf-8")?;
            if method == "HEAD" {
                return stream.write_all(b"\r\n");
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
                let name = String::from_utf8_lossy(&cache[line_start..i])
                    .trim().to_ascii_lowercase();
                if name != "content-length" {
                    continue;
                }
                let value = String::from_utf8_lossy(&cache[i+header_sb.length..line_end])
                    .trim().to_owned();
                // println!("H '{}': '{}'", name, value);
                content_length = match value.parse() {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Error parsing Content-Length str (\"{}\") to int: {}", value, e);
                        0
                    }
                };
                break;
            }
        }
    }


    while !eof && line_sb.result() != Some(0) {
        line_sb.reset();
        loop {
            if stream.read(&mut buf)? == 0 {
                eof = true;
                break;
            }
            let byte = &buf[0];
            cache.push(*byte);

            if line_sb.search(byte).is_some() {
                break;
            }
        }
    }

    stream.write_header(b"Content-Length",
                        (12 + cache.len() + content_length).to_string().as_bytes())?;
    stream.write_new_line()?;
    stream.write_all(b"Hello HTTP\n\n")?;
    stream.write_all(&cache)?;
    drop(cache);

    const BUFFER_SIZE: usize = 1024;
    let mut buffer = [0; BUFFER_SIZE];
    let mut total = 0;
    while total < content_length {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        stream.write_all(&buffer[..n])?;
        total += n;
    }
    Ok(())
}

fn main() {

    unsafe {
        A_HOST = Some("127.0.0.1".to_string());
        A_PORT = Some(8080);
    }

    parse_args();

    #[allow(static_mut_refs)]
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
                    match handle_tcp_stream(stream) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error handling TcpStream: {}", e);
                        }
                    };
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}