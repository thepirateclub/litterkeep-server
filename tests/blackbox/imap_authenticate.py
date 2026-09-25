"""Проверяет IMAP AUTHENTICATE PLAIN continuation и SASL-IR через STARTTLS."""

import argparse
import base64
import imaplib
import ssl
from pathlib import Path


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    return parser.parse_args()


def password_for(path: Path, username: str) -> str:
    """Извлекает пароль пользователя из локального acceptance-файла."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [name, password] if name == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing acceptance account: {username}")


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def open_tls(port: int, context: ssl.SSLContext, plain_response: bytes) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    before = b" ".join(require(client.capability(), "OK"))
    if b"LOGINDISABLED" not in before or b"AUTH=PLAIN" in before:
        raise AssertionError(f"unsafe pre-TLS capability: {before!r}")
    denied = client._simple_command(
        "AUTHENTICATE", "PLAIN", base64.b64encode(plain_response).decode("ascii")
    )
    match denied:
        case ("NO", details) if b"PRIVACYREQUIRED" in b" ".join(details):
            pass
        case _:
            raise AssertionError(f"plaintext authentication was not denied: {denied!r}")
    require(client.starttls(ssl_context=context), "OK")
    after = b" ".join(require(client.capability(), "OK"))
    for capability in (b"AUTH=PLAIN", b"SASL-IR"):
        if capability not in after:
            raise AssertionError(f"missing {capability!r}: {after!r}")
    return client


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    password = password_for(options.credentials, "bob")
    plain_response = b"\0bob\0" + password.encode("utf-8")
    continuation = open_tls(options.port, context, plain_response)
    require(
        continuation.authenticate("PLAIN", lambda _: plain_response),
        "OK",
    )
    require(continuation.noop(), "OK")
    continuation.logout()

    initial = open_tls(options.port, context, plain_response)
    payload = base64.b64encode(plain_response).decode("ascii")
    require(initial._simple_command("AUTHENTICATE", "PLAIN", payload), "OK")
    require(initial.noop(), "OK")
    initial.logout()

    quoted = open_tls(options.port, context, plain_response)
    require(quoted._simple_command('AUTHENTICATE', '"PLAIN"', f'"{payload}"'), "OK")
    require(quoted.noop(), "OK")
    quoted.logout()

    empty = open_tls(options.port, context, plain_response)
    rejected = empty._simple_command("AUTHENTICATE", "PLAIN", "=")
    match rejected:
        case ("NO", _):
            pass
        case _:
            raise AssertionError(f"empty SASL-IR must fail credentials, got {rejected!r}")
    require(empty.noop(), "OK")
    empty.logout()
    print("IMAP AUTHENTICATE passed: continuation, SASL-IR, quoted astring, empty marker")


if __name__ == "__main__":
    main()
