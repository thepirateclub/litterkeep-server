"""Проверяет RFC 5322 address groups через IMAP ENVELOPE и SEARCH."""

import argparse
import imaplib
import os
import ssl
import uuid


def require(result: tuple[str, object], operation: str) -> object:
    """Завершает acceptance на любом tagged status, отличном от OK."""
    status, data = result
    if status != "OK":
        raise AssertionError(f"{operation}: {status} {data!r}")
    return data


def response_bytes(data: object) -> bytes:
    """Объединяет непрерывные response chunks без добавления разделителей."""
    match data:
        case bytes():
            return data
        case tuple() | list():
            return b"".join(response_bytes(part) for part in data)
        case _:
            return b""


def search_address(client: imaplib.IMAP4, field: str, value: str) -> bytes:
    """Требует, чтобы ENVELOPE-проекция нашла заданный group/mailbox term."""
    result = require(client.search(None, field, f'"{value}"'), f"SEARCH {field}")
    match result:
        case [only]:
            return only
        case _:
            raise AssertionError(f"SEARCH {field} {value!r}: {result!r}")


def require_no_search_match(client: imaplib.IMAP4, field: str, value: str) -> None:
    """Подтверждает, что комментарии не становятся содержимым адреса для SEARCH."""
    result = require(client.search(None, field, f'"{value}"'), f"SEARCH {field}")
    match result:
        case [b""]:
            return
        case _:
            raise AssertionError(f"SEARCH {field} unexpectedly matched {value!r}: {result!r}")


def verify_legacy_quoted_control_literal(client: imaplib.IMAP4) -> None:
    """Проверяет RFC 5322 obs-qp в адресе и IMAP literal в ENVELOPE."""
    mailbox = f"EnvelopeLiteral-{uuid.uuid4().hex}"
    require(client.create(mailbox), "CREATE literal mailbox")
    message = (
        b"From: Literal <literal@[host\\"
        + b"\0tail]>\r\nSubject: obsolete quoted pair\r\n\r\nbody\r\n"
    )
    require(client.append(mailbox, None, None, message), "APPEND quoted control")
    require(client.select(mailbox), "SELECT literal mailbox")
    result = require(client.fetch("1", "(ENVELOPE)"), "FETCH literal ENVELOPE")
    wire = response_bytes(result)
    domain = b"[host\0tail]"
    # imaplib извлекает synchronizing literal и убирает framing CRLF из chunks.
    literal = b"{" + str(len(domain)).encode("ascii") + b"}" + domain
    if literal not in wire:
        raise AssertionError(f"quoted control was not returned as a literal: {wire!r}")


def enable_utf8_accept(client: imaplib.IMAP4) -> None:
    """Включает RFC 9755 без устаревшего APPEND data item из imaplib."""
    status, _ = client._simple_command("ENABLE", "UTF8=ACCEPT")
    response_type, enabled = client.response("ENABLED")
    if status != "OK":
        raise AssertionError(f"ENABLE failed: {status!r}")
    if response_type != "ENABLED":
        raise AssertionError(f"ENABLE response is absent: {response_type!r}")
    if not any(b"UTF8=ACCEPT" in item for item in enabled):
        raise AssertionError(f"UTF8=ACCEPT was not enabled: {enabled!r}")
    client._encoding = "utf-8"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2152)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=context)
    require(client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"]), "LOGIN")
    enable_utf8_accept(client)
    mailbox = f"EnvelopeGroups-{uuid.uuid4().hex}"
    group = f"A-Group-{uuid.uuid4().hex}"
    require(client.create(mailbox), "CREATE")
    message = (
        r'From: "Alice \"Sender\"" (primary (nested)) <alice@example.test>, '
        "=?UTF-8?Q?From?= =?UTF-8?Q?_Phrase?= <from@example.test>\r\n"
        "Sender: =?UTF-8?Q?Encoded?= =?UTF-8?Q?_Name?= <sender@example.test>\r\n"
        'Reply-To: "=?UTF-8?Q?Literal?=" <reply@example.test>\r\n'
        f"To: , {group} (list comment):Ed Jones (mentor (owner \\) note)) "
        "<c(comment)@a.test>,,joe@where.test,John <jdoe@one.test>, "
        'Quoted Local <"a@b"@example.test>, '
        "Legacy <@old.example,@older.example,:route-target@example.test>, "
        "Old Dots <first (local) . (separator) last @ example . test>; "
        "(group closed), Unicode Alice\u00a0North <unicode@example.test>\r\n"
        "Cc: Undisclosed recipients:;\r\n"
        "Bcc: Hidden Team:hidden@example.test;\r\n"
        "Subject: address group fixture\r\n\r\nbody\r\n"
    )
    require(client.append(mailbox, None, None, message.encode("utf-8")), "APPEND")
    require(client.select(mailbox), "SELECT")
    envelope = require(client.fetch("1", "(ENVELOPE)"), "FETCH ENVELOPE")
    wire = response_bytes(envelope)
    expected = [
        f'(NIL NIL "{group}" NIL)'.encode("ascii"),
        br'("Alice \"Sender\"" NIL "alice" "example.test")',
        b'("From Phrase" NIL "from" "example.test")',
        b'("Encoded Name" NIL "sender" "example.test")',
        b'("=?UTF-8?Q?Literal?=" NIL "reply" "example.test")',
        b'("Ed Jones" NIL "c" "a.test")',
        b'(NIL NIL "joe" "where.test")',
        b'("John" NIL "jdoe" "one.test")',
        b'("Quoted Local" NIL "\\"a@b\\"" "example.test")',
        b'("Legacy" NIL "route-target" "example.test")',
        b'("Old Dots" NIL "first.last" "example.test")',
        '("Unicode Alice\u00a0North" NIL "unicode" "example.test")'.encode(
            "utf-8"
        ),
        b'(NIL NIL NIL NIL)',
        b'(NIL NIL "Undisclosed recipients" NIL)',
        b'(NIL NIL "Hidden Team" NIL)',
    ]
    for marker in expected:
        if marker not in wire:
            raise AssertionError(f"ENVELOPE group marker missing {marker!r}: {wire!r}")
    comment_terms = (
        b"mentor",
        b"list comment",
        b"owner",
        b"group closed",
        b"old.example",
        b"older.example",
        b"local",
        b"separator",
    )
    if any(term in wire for term in comment_terms):
        raise AssertionError(f"RFC 5322 comments leaked into ENVELOPE: {wire!r}")
    if search_address(client, "TO", group) != b"1":
        raise AssertionError("TO search did not match group phrase")
    from_match = search_address(client, "FROM", "From Phrase")
    if from_match != b"1":
        raise AssertionError(f"FROM search did not use RFC 2047 phrase decoding: {from_match!r}")
    if search_address(client, "TO", "jdoe@one.test") != b"1":
        raise AssertionError("TO search did not match a mailbox inside the group")
    if search_address(client, "TO", "a@b") != b"1":
        raise AssertionError("TO search did not preserve @ inside a quoted local-part")
    if search_address(client, "TO", "route-target@example.test") != b"1":
        raise AssertionError("TO search did not preserve the source-route destination")
    if search_address(client, "TO", "first.last@example.test") != b"1":
        raise AssertionError("TO search did not normalize obsolete CFWS around dots")
    if search_address(client, "CC", "Undisclosed recipients") != b"1":
        raise AssertionError("CC search did not match the empty group name")
    if search_address(client, "BCC", "hidden@example.test") != b"1":
        raise AssertionError("BCC search did not match a mailbox inside the group")
    require_no_search_match(client, "TO", "mentor")
    require_no_search_match(client, "TO", "list comment")
    verify_legacy_quoted_control_literal(client)
    client.logout()
    print(
        "PASS imap-envelope-groups: comments, groups, quoted local-part, "
        "source-route, address SEARCH"
    )


if __name__ == "__main__":
    main()
