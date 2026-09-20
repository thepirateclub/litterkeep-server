"""Проверяет IMAP AUTHENTICATE PLAIN continuation и SASL-IR через STARTTLS."""

import argparse
import base64
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def open_tls(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    before = b" ".join(require(client.capability(), "OK"))
    if b"LOGINDISABLED" not in before or b"AUTH=PLAIN" in before:
        raise AssertionError(f"unsafe pre-TLS capability: {before!r}")
    require(client.starttls(ssl_context=context), "OK")
    after = b" ".join(require(client.capability(), "OK"))
    for capability in (b"AUTH=PLAIN", b"SASL-IR"):
        if capability not in after:
            raise AssertionError(f"missing {capability!r}: {after!r}")
    return client


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    continuation = open_tls(options.port, context)
    require(
        continuation.authenticate("PLAIN", lambda _: b"\0bob\0bob-password"),
        "OK",
    )
    require(continuation.noop(), "OK")
    continuation.logout()

    initial = open_tls(options.port, context)
    payload = base64.b64encode(b"\0bob\0bob-password").decode("ascii")
    require(initial._simple_command("AUTHENTICATE", "PLAIN", payload), "OK")
    require(initial.noop(), "OK")
    initial.logout()
    print("IMAP AUTHENTICATE passed: continuation and SASL-IR")


if __name__ == "__main__":
    main()
