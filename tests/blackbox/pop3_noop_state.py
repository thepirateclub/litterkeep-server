"""Проверяет состояния RFC 1939 NOOP независимым POP3 STARTTLS peer."""

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


def exchange(stream, command, prefix):
    stream.write(command + b"\r\n")
    response = read_line(stream)
    if not response.startswith(prefix):
        raise AssertionError(f"{command!r}: ожидалось {prefix!r}, получено {response!r}")
    return response


def run(port):
    password = os.environ["LITTERKEEP_BOB_PASSWORD"].encode("ascii")
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        stream = connection.makefile("rwb", buffering=0)
        if not read_line(stream).startswith(b"+OK"):
            raise AssertionError("POP3 greeting не положительный")
        exchange(stream, b"STLS", b"+OK")
        stream.close()
        with context.wrap_socket(connection, server_hostname="localhost") as tls:
            stream = tls.makefile("rwb", buffering=0)
            exchange(stream, b"NOOP", b"-ERR")
            exchange(stream, b"USER bob", b"+OK")
            exchange(stream, b"NOOP", b"-ERR")
            failed_password = exchange(stream, b"PASS " + b"p" * 40, b"-ERR")
            if not failed_password.startswith(b"-ERR authentication failed"):
                raise AssertionError(f"USER не сохранился после NOOP: {failed_password!r}")
            sasl = base64.b64encode(b"\x00bob\x00" + password)
            exchange(stream, b"AUTH PLAIN " + sasl, b"+OK")
            exchange(stream, b"NOOP", b"+OK")
            exchange(stream, b"QUIT", b"+OK")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    run(parser.parse_args().port)
    print("PASS POP3 RFC 1939 NOOP transaction-state restriction")


if __name__ == "__main__":
    main()
