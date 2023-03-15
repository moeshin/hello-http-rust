use std::collections::HashSet;
use std::{env, io, process};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

static mut A_HOST: Option<String> = None;
static mut A_PORT: Option<u16> = None;
static mut A_ALLOWED_METHODS: Option<HashSet<String>> = None;
static mut A_DISALLOWED_METHODS: Option<HashSet<String>> = None;

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
    return if set.len() == 0 {
        None
    } else {
        Some(set)
    };
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

fn handle_tcp_stream(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let mut headers_end_flag: u8 = 0;
    let mut is_headers_end = false;
    let mut start_line = Vec::new();
    let mut is_start_line_end = false;
    let mut has_method = false;
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(mut n) => {
                for i in 0..n {
                    let byte = buffer[i];
                    if !is_start_line_end {
                        start_line.push(byte);
                        if !has_method && byte == b' ' {
                            has_method = true;

                            let method = String::from_utf8_lossy(
                                &start_line[..start_line.len()])
                                .trim_start().to_ascii_uppercase();
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
                                he_write(stream.write("\
                                HTTP/1.1 405 Method not allowed\
                                \r\n\r\n".as_bytes()));
                                return
                            }
                            he_write(stream.write("\
                            HTTP/1.1 200 OK\r\n\
                            Content-Type: text/plain; charset=utf-8\r\n\
                            \r\n".as_bytes()));
                            if method == "HEAD" {
                                continue;
                            }
                            he_write(stream.write("Hello: ".as_bytes()));
                        }
                    }
                    if headers_end_flag == 0 && byte == b'\r' {
                        headers_end_flag = 1;
                        continue;
                    }
                    let crlf_mod = headers_end_flag % 2;
                    if !(crlf_mod == 0 && byte == b'\r') && !(crlf_mod == 1 && byte == b'\n') {
                        headers_end_flag = 0;
                        if byte == b'\r' {
                            headers_end_flag = 1;
                        }
                        continue;
                    }
                    if !is_start_line_end && crlf_mod == 1 {
                        is_start_line_end = true;
                        unsafe {
                            start_line.set_len(start_line.len() - 2);
                        }
                        let start_line = String::from_utf8_lossy(&start_line);
                        println!("{}", start_line);
                    }
                    if headers_end_flag > 2 {
                        n = i - 3;
                        is_headers_end = true;
                        break;
                    }
                    headers_end_flag += 1;
                }
                he_write(stream.write(&buffer[0..n]));
                if is_headers_end {
                    break;
                }
            },
            Err(e) => {
                eprintln!("Error reading from TcpStream: {}", e);
                break;
            }
        }
    }

    stream.flush().unwrap();
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
