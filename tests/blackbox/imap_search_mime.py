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
    for subject in (b"secondary-two", b"secondary-three"):
        require(
            client.append(
                mailbox,
                None,
                None,
                b"Subject: " + subject + b"\r\n\r\nsecondary body\r\n",
            ),
            "OK",
        )
    require(client.select(mailbox), "OK")
    saved_search = raw_search(
        client,
        b"RETURN",
        b"(SAVE)",
        b"CHARSET",
        b"UTF-8",
        b"HEADER",
        b"SUBJECT",
        quoted("тема"),
    )
    assert saved_search == ("OK", []), saved_search
    _, saved_esearch = client.response("ESEARCH")
    assert not [item for item in (saved_esearch or []) if item is not None]
    saved_sequence = require(raw_search(client, b"$"), "OK")
    assert saved_sequence == [b"1"], saved_sequence
    saved_uid = require(client.uid("SEARCH", "$"), "OK")
    assert saved_uid == [b"1"], saved_uid
    other = imaplib.IMAP4(options.host, options.port, timeout=10)
    other.starttls(ssl_context=context)
    require(other.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"]), "OK")
    require(other.select(mailbox), "OK")
    isolated_search = require(raw_search(other, b"$"), "OK")
    assert isolated_search == [b""], isolated_search
    other.logout()
    min_max_save = raw_search(client, b"RETURN", b"(SAVE MIN MAX)", b"ALL")
    assert min_max_save == ("OK", []), min_max_save
    _, min_max_items = client.response("ESEARCH")
    min_max_text = b" ".join(item for item in (min_max_items or []) if item is not None)
    assert b"MIN 1" in min_max_text and b"MAX 3" in min_max_text, min_max_text
    min_max_search = require(raw_search(client, b"$"), "OK")
    assert min_max_search == [b"1 3"], min_max_search
    require(
        raw_search(
            client,
            b"RETURN",
            b"(SAVE)",
            b"CHARSET",
            b"UTF-8",
            b"HEADER",
            b"SUBJECT",
            quoted("тема"),
        ),
        "OK",
    )
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
    unknown_cte = (
        b"Content-Type: text/plain; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: x-private\r\n\r\n"
        b"searchable-after-decode-error"
    )
    require(client.append(mailbox, None, None, unknown_cte), "OK")
    tag = client._command("SEARCH", b"BODY", quoted("searchable-after-decode-error"))
    status, detail = client._command_complete("SEARCH", tag)
    assert status == "NO", (status, detail)
    assert b"UNKNOWN-CTE" in b" ".join(detail), detail
    _, response_items = client.response("SEARCH")
    search_result = b" ".join(
        item for item in (response_items or []) if item is not None
    ).strip()
    assert not search_result, search_result
    preserved_search = require(raw_search(client, b"$"), "OK")
    assert preserved_search == [b"1"], preserved_search
    tag = client._command(
        "SEARCH", b"RETURN", b"(SAVE)", b"BODY", quoted("searchable-after-decode-error")
    )
    status, detail = client._command_complete("SEARCH", tag)
    assert status == "NO", (status, detail)
    assert b"UNKNOWN-CTE" in b" ".join(detail), detail
    cleared_search = require(raw_search(client, b"$"), "OK")
    assert cleared_search == [b""], cleared_search
    require(
        raw_search(
            client,
            b"RETURN",
            b"(SAVE)",
            b"CHARSET",
            b"UTF-8",
            b"HEADER",
            b"SUBJECT",
            quoted("тема"),
        ),
        "OK",
    )
    require(client.select(mailbox), "OK")
    reset_search = require(raw_search(client, b"$"), "OK")
    assert reset_search == [b""], reset_search
    client.logout()
    print("PASS imap-search-mime")


def raw_search(client: imaplib.IMAP4, *arguments: bytes) -> tuple[str, list[bytes]]:
    """Отправляет Unicode criteria как UTF-8 octets без ASCII conversion."""
    tag = client._command("SEARCH", *arguments)
    status, data = client._command_complete("SEARCH", tag)
    _, response_items = client.response("SEARCH")
    return status, [item for item in (response_items or []) if item is not None]


def quoted(value: str) -> bytes:
    return b'"' + value.encode("utf-8") + b'"'


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


if __name__ == "__main__":
    main()
