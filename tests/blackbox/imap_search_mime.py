"""Проверяет decoded BODY/TEXT и encoded-word HEADER по STARTTLS."""

import argparse
import base64
import imaplib
import os
import quopri
import ssl
import uuid


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=context)
    require(client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"]), "OK")
    mailbox = f"MimeSearch-{uuid.uuid4().hex}"
    require(client.create(mailbox), "OK")
    encoded_subject = base64.b64encode("Привет тема".encode("utf-8"))
    encoded_description = base64.b64encode("метаданные".encode("utf-8"))
    encoded_body = base64.b64encode("Привет body".encode("utf-8"))
    qp_body = quopri.encodestring("café q".encode("utf-8"))
    attachment = base64.b64encode(b"hidden-attachment-token")
    message = (
        b"Subject: =?UTF-8?B?" + encoded_subject + b"?=\r\n"
        b"Content-Type: multipart/mixed; boundary=search-boundary\r\n\r\n"
        b"--search-boundary\r\n"
        b"Content-Description: =?UTF-8?B?"
        + encoded_description
        + b"?=\r\n"
        b"Content-Type: text/plain; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: base64\r\n\r\n"
        + encoded_body
        + b"\r\n--search-boundary\r\n"
        b"Content-Type: text/calendar; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: quoted-printable\r\n\r\n"
        + qp_body
        + b"\r\n--search-boundary\r\n"
        b"Content-Type: application/octet-stream\r\n"
        b"Content-Transfer-Encoding: base64\r\n\r\n"
        + attachment
        + b"\r\n--search-boundary--\r\n"
    )
    require(client.append(mailbox, None, None, message), "OK")
    require(client.select(mailbox), "OK")
    body_unicode = require(
        raw_search(client, b"CHARSET", b"UTF-8", b"BODY", quoted("Привет")),
        "OK",
    )
    assert body_unicode == [b"1"], body_unicode
    body_qp = require(
        raw_search(client, b"CHARSET", b"UTF-8", b"BODY", quoted("café")), "OK"
    )
    assert body_qp == [b"1"], body_qp
    mime_header = require(
        raw_search(
            client,
            b"CHARSET",
            b"UTF-8",
            b"BODY",
            quoted("метаданные"),
        ),
        "OK",
    )
    assert mime_header == [b"1"], mime_header
    header_encoded_word = require(
        raw_search(
            client,
            b"CHARSET",
            b"UTF-8",
            b"HEADER",
            b"SUBJECT",
            quoted("тема"),
        ),
        "OK",
    )
    assert header_encoded_word == [b"1"], header_encoded_word
    body_header_term = raw_search(
        client, b"CHARSET", b"UTF-8", b"BODY", quoted("тема")
    )
    body_header_term = require(body_header_term, "OK")
    assert body_header_term == [b""], body_header_term
    text_header_term = require(
        raw_search(client, b"CHARSET", b"UTF-8", b"TEXT", quoted("тема")), "OK"
    )
    assert text_header_term == [b"1"], text_header_term
    attachment_term = raw_search(
        client,
        b"CHARSET",
        b"UTF-8",
        b"BODY",
        quoted("hidden-attachment-token"),
    )
    attachment_term = require(attachment_term, "OK")
    assert attachment_term == [b""], attachment_term
    client.logout()
    print("PASS imap-search-mime")


def raw_search(client: imaplib.IMAP4, *arguments: bytes) -> tuple[str, list[bytes]]:
    """Отправляет Unicode criteria как UTF-8 octets без ASCII conversion."""
    tag = client._command("SEARCH", *arguments)
    status, data = client._command_complete("SEARCH", tag)
    result = client.response("SEARCH")
    return status, result[1] or []


def quoted(value: str) -> bytes:
    return b'"' + value.encode("utf-8") + b'"'


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


if __name__ == "__main__":
    main()
