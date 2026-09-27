"""Проверяет RFC 1939 USER/PASS размер на независимом POP3 STARTTLS peer."""

import argparse
import base64
import os
import socket
import ssl


def read_line(stream):
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"POP3 ответ не завершён CRLF: {line!r}")
    return line


def expect(stream, prefix):
    response = read_line(stream)
    if not response.startswith(prefix):
        raise AssertionError(f"ожидалось {prefix!r}, получено {response!r}")
    return response


def run(port):
    password = os.environ["LITTERKEEP_BOB_PASSWORD"].encode("ascii")
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        stream = connection.makefile("rwb", buffering=0)
        expect(stream, b"+OK")
        stream.write(b"STLS\r\n")
        expect(stream, b"+OK")
        stream.close()
        tls = context.wrap_socket(connection, server_hostname="localhost")
        stream = tls.makefile("rwb", buffering=0)

        stream.write(b"USER " + b"u" * 41 + b"\r\n")
        expect(stream, b"-ERR")
        stream.write(b"USER bob\r\n")
        expect(stream, b"+OK")
        stream.write(b"PASS " + b"p" * 41 + b"\r\n")
        expect(stream, b"-ERR")
        sasl = base64.b64encode(b"\x00bob\x00" + password)
        stream.write(b"AUTH PLAIN " + sasl + b"\r\n")
        expect(stream, b"+OK")
        stream.write(b"STAT\r\n")
        expect(stream, b"+OK")
        stream.write(b"QUIT\r\n")
        expect(stream, b"+OK")
        tls.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    run(parser.parse_args().port)
    print("PASS POP3 RFC 1939 USER/PASS argument length boundaries")


if __name__ == "__main__":
    main()
