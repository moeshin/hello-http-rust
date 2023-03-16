# Hello HTTP

Build an HTTP server based on the standard library and respond to the request message.

## Usage

```text
Usage: hello-http [options]
Options:
  -h, --host <host>
        Listen host. (default "127.0.0.1")
  -p, --port <port>
        Listen port. If 0 is random. (default 8080)
  -m, --allowed-methods <method>[,<methods>...]
        Disallowed methods.
  -d, --disallowed-methods <method>[,<methods>...]
        Allowed methods.
  --help
        Print help.
```

Run

```shell
./hello-http
```

Hello HTTP output

```text
Listening 127.0.0.1:8080
```

Test by cURL

```shell
curl http://127.0.0.1:8080
```

cURL output

```text
Hello HTTP

GET / HTTP/1.1
Host: 127.0.0.1:8080
User-Agent: curl/7.74.0
Accept: */*

```

Hello HTTP output

```text
GET / HTTP/1.1
```

## Notes

* Listen IPv4 and IPv6 at the same time use `-h ::`.  
However, it is not possible to listen together on Windows.
* Chunked transfer not implemented.
