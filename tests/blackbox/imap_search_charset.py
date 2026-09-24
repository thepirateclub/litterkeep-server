"""Независимая проверка RFC 9051 SEARCH charset по STARTTLS."""
import argparse
import imaplib
import os
import ssl
import uuid


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
OPTIONS = PARSER.parse_args()
BOB_PASSWORD = os.environ.get("LITTERKEEP_BOB_PASSWORD", "test-password")
TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE


def mailbox_message(marker):
    """Собирает octet-preserving RFC 5322 fixture с UTF-8 текстовым телом."""
    body = f"charset test {marker}: café 東京"
    return (
        "From: alice@test\r\n"
        "To: bob@test\r\n"
        f"Subject: charset-test-{marker}\r\n"
        "Content-Type: text/plain; charset=utf-8\r\n"
        "Content-Transfer-Encoding: 8bit\r\n"
        "\r\n"
        f"{body}\r\n"
    ).encode("utf-8")


def raw_search(connection, *arguments):
    """Отправляет SEARCH octets без перекодирования ASCII-only imaplib."""
    tag = connection._command("SEARCH", *arguments)
    return connection._command_complete("SEARCH", tag)


with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
    imap.starttls(ssl_context=TLS_CONTEXT)
    status, _ = imap.login("bob", BOB_PASSWORD)
    assert status == "OK", status
    status, _ = imap.select("INBOX")
    assert status == "OK", status

    marker = uuid.uuid4().hex
    status, _ = imap.append("INBOX", None, None, mailbox_message(marker))
    assert status == "OK", status

    status, ascii_matches = imap.search("US-ASCII", "ALL")
    assert status == "OK", (status, ascii_matches)
    status, utf8_matches = raw_search(
        imap, "CHARSET", "UTF-8", "BODY", b'"caf\xc3\xa9"'
    )
    assert status == "OK", (status, utf8_matches)
    assert next(iter(utf8_matches)).strip(), utf8_matches

    status, default_matches = raw_search(imap, "BODY", b'"\xe6\x9d\xb1\xe4\xba\xac"')
    assert status == "OK", (status, default_matches)
    assert next(iter(default_matches)).strip(), default_matches

    status, unsupported = imap.search("KOI8-R", "ALL")
    assert status == "NO", (status, unsupported)
    assert any(b"BADCHARSET" in response for response in unsupported), unsupported

    status, mismatch = raw_search(
        imap, "CHARSET", "US-ASCII", "BODY", b'"caf\xc3\xa9"'
    )
    assert status == "NO", (status, mismatch)

print("PASS imap-search-charset")
