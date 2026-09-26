"""Независимо проверяет передачу пробелов в POP3 PASS через STARTTLS."""

import argparse
import socket
import ssl


def response(stream, prefix: bytes, name: str) -> bytes:
    """Читает одну POP3 строку и проверяет ожидаемый статусный префикс."""
    line = stream.readline()
    if not line.endswith(b"\r\n") or not line.startswith(prefix):
        raise AssertionError(f"{name}: unexpected POP3 response {line!r}")
    return line


def verify(host: str, port: int, ca: str) -> None:
    """Доказывает, что PASS с пробелами достигает auth owner и восстанавливает FSM."""
    context = ssl.create_default_context(cafile=ca)
    connection = socket.create_connection((host, port), timeout=10)
    stream = connection.makefile("rb")
    response(stream, b"+OK", "greeting")
    connection.sendall(b"STLS\r\n")
    response(stream, b"+OK", "STLS")
    stream.close()
    connection = context.wrap_socket(connection, server_hostname="localhost")
    stream = connection.makefile("rb")

    connection.sendall(b"USER bob\r\n")
    response(stream, b"+OK", "USER")
    connection.sendall(b"PASS deliberately wrong password\r\n")
    failure = response(stream, b"-ERR", "PASS with spaces")
    if failure != b"-ERR authentication failed\r\n":
        raise AssertionError(f"PASS did not reach authentication owner: {failure!r}")

    connection.sendall(b"USER bob\r\n")
    response(stream, b"+OK", "USER after authentication failure")
    connection.sendall(b"QUIT\r\n")
    response(stream, b"+OK", "QUIT")
    stream.close()
    connection.close()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    options = parser.parse_args()
    verify(options.host, options.port, options.ca)
    print("PASS POP3 PASS argument spaces reach the authentication owner")


if __name__ == "__main__":
    main()
