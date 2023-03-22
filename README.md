# Hello HTTP

Build an HTTP server based on the standard library and respond to the request message.

## Usage

```text
Usage: hello-http [options]

Options:
  -h <host>
        Listen host.
        (default "127.0.0.1")
  -p <port>
        Listen port.
        If 0 is random.
        (default 8080)
  -m <method>[,<method>...]
        Disallowed methods.
  -d <method>[,<method>...]
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

Test with cURL

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

* If your operating system has dual-stack sockets enabled, you will be able to listen on both IPv4 and IPv6.
* Chunked transfer not implemented.
