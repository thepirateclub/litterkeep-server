"""Независимая SMTP -> POP3/STLS проверка одного durable INBOX."""

import argparse
from email.message import EmailMessage
from email.parser import BytesParser
from email.policy import default
from itertools import islice
import os
import poplib
import smtplib
import ssl
import time


ATTACHMENT = b"\x00litterkeep-pop3-attachment\xff\x10"


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("smtp", "pop3", "empty", "lease", "implicit", "rset", "disconnect", "deleted-visibility", "top", "lang"), required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--smtp-port", type=int, default=2525)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--account", default="bob")
    parser.add_argument("--password", default="bob-password")
    return parser.parse_args()


def context(ca: str) -> ssl.SSLContext:
    return ssl.create_default_context(cafile=ca)


def smtp(port: int, tls: ssl.SSLContext, account: str) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = f"{account}@litterkeep.test"
    message["Subject"] = "POP3 durable attachment"
    message.set_content("POP3 acceptance message")
    message.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="proof.bin",
    )
    with smtplib.SMTP("127.0.0.1", port) as client:
        client.starttls(context=tls)
        client.login("alice", os.environ.get("LITTERKEEP_ALICE_PASSWORD", "alice-password"))
        client.send_message(message)
    print("SMTP delivery passed")


def pop3(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
    count, _ = client.stat()
    if count != 1:
        raise AssertionError(f"expected one delivered message, got {count}")
    _, uidl_lines, _ = client.uidl()
    if b"lk-" not in b"\n".join(uidl_lines):
        raise AssertionError(f"durable UIDL missing: {uidl_lines!r}")
    _, lines, _ = client.retr(1)
    parsed = BytesParser(policy=default).parsebytes(b"\r\n".join(lines) + b"\r\n")
    attachments = tuple(parsed.iter_attachments())
    attachment = next(iter(attachments), None)
    if attachment is None or attachment.get_payload(decode=True) != ATTACHMENT:
        raise AssertionError("POP3 RETR attachment bytes differ")
    client.dele(1)
    client.quit()
    print("POP3 STLS passed: UIDL, RETR attachment bytes and durable QUIT")


def empty(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
    count, _ = client.stat()
    client.quit()
    if count != 0:
        raise AssertionError(f"QUIT deletion did not survive restart: {count}")
    print("POP3 restart replay passed")


def lease(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    first = authenticated(port, tls, account, password)
    second = poplib.POP3("127.0.0.1", port)
    second.stls(context=tls)
    second.user(account)
    try:
        second.pass_(password)
    except poplib.error_proto as error:
        if "maildrop is locked" not in str(error):
            raise
    else:
        raise AssertionError("concurrent POP3 session received the same maildrop")
    first.close()
    time.sleep(0.2)
    second.pass_(password)
    second.quit()
    print("POP3 maildrop lease passed: concurrent denial and EOF release")


def implicit(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3_SSL("127.0.0.1", port, context=tls)
    client.user(account)
    client.pass_(password)
    client.quit()
    print("POP3 implicit TLS passed")


def authenticated(port: int, tls: ssl.SSLContext, account: str, password: str) -> poplib.POP3:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
    return client


def listed_uid(line: bytes) -> bytes:
    """Извлекает только стабильный UID из одной строки UIDL-ответа."""
    _, separator, uid = line.partition(b" ")
    if separator != b" ":
        raise AssertionError(f"malformed UIDL listing: {line!r}")
    return uid


def rset(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = authenticated(port, tls, account, password)
    client.dele(1)
    client.rset()
    client.quit()
    replay = authenticated(port, tls, account, password)
    count, _ = replay.stat()
    replay.quit()
    if count != 1:
        raise AssertionError(f"RSET failed to retain the message: {count}")
    print("POP3 RSET passed: DELE marker was not committed")


def disconnect(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = authenticated(port, tls, account, password)
    client.dele(1)
    client.close()
    time.sleep(0.2)
    replay = authenticated(port, tls, account, password)
    count, _ = replay.stat()
    replay.quit()
    if count != 1:
        raise AssertionError(f"disconnect committed a POP3 DELE marker: {count}")
    print("POP3 disconnect passed: DELE marker was not committed")


def deleted_visibility(
    port: int,
    smtp_port: int,
    tls: ssl.SSLContext,
    account: str,
    password: str,
) -> None:
    """Сверяет POP3-видимость DELE до RSET и после durable QUIT."""
    smtp(smtp_port, tls, account)
    smtp(smtp_port, tls, account)
    client = authenticated(port, tls, account, password)
    initial_count, _ = client.stat()
    if initial_count != 2:
        raise AssertionError(f"expected two messages, got {initial_count}")
    _, initial_list, _ = client.list()
    _, initial_uidl, _ = client.uidl()
    _, second_listing = initial_list
    _, second_uidl = initial_uidl
    client.dele(1)
    try:
        client.dele(1)
    except poplib.error_proto as error:
        response = next(iter(error.args), b"")
        if not isinstance(response, bytes) or not response.startswith(b"-ERR"):
            raise AssertionError(f"duplicate DELE returned an invalid error: {error!r}") from error
    else:
        raise AssertionError("duplicate DELE unexpectedly succeeded")
    count, _ = client.stat()
    _, remaining_list, _ = client.list()
    _, remaining_uidl, _ = client.uidl()
    if (count, remaining_list, remaining_uidl) != (
        1,
        [second_listing],
        [second_uidl],
    ):
        raise AssertionError("DELE did not hide the same message from STAT/LIST/UIDL")
    client.rset()
    _, reset_list, _ = client.list()
    _, reset_uidl, _ = client.uidl()
    if (reset_list, reset_uidl) != (initial_list, initial_uidl):
        raise AssertionError("RSET did not restore deleted message visibility")
    client.dele(1)
    client.quit()
    replay = authenticated(port, tls, account, password)
    count, _ = replay.stat()
    _, replay_uidl, _ = replay.uidl()
    replay.quit()
    remaining_ids = [listed_uid(line) for line in replay_uidl]
    expected_uid = listed_uid(second_uidl)
    if (count, remaining_ids) != (1, [expected_uid]):
        raise AssertionError("QUIT did not durably delete only the marked message")
    print("POP3 DELE visibility passed: STAT/LIST/UIDL hide markers until RSET or QUIT")


def top_semantics(port: int, smtp_port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    """Сверяет TOP 0, точное число строк, dot-stuffing и конец короткого body."""
    message = (
        f"From: alice@litterkeep.test\r\nTo: {account}@litterkeep.test\r\n"
        "Subject: POP3 TOP exact\r\n\r\n.first\r\n..second\r\nthird\r\n"
    ).encode("ascii")
    with smtplib.SMTP("127.0.0.1", smtp_port, timeout=10) as client:
        client.starttls(context=tls)
        client.login("alice", os.environ.get("LITTERKEEP_ALICE_PASSWORD", "alice-password"))
        refused = client.sendmail("alice@litterkeep.test", [f"{account}@litterkeep.test"], message)
        if refused:
            raise AssertionError(f"SMTP rejected TOP fixture: {refused!r}")

    client = authenticated(port, tls, account, password)
    _, headers, _ = client.top(1, 0)
    _, first_body, _ = client.top(1, 1)
    _, complete_body, _ = client.top(1, 9)
    header_count = len(headers)
    expected_body = [b".first", b"..second", b"third"]
    last_header = next(reversed(headers), None)
    first_returned_line = next(reversed(first_body), None)
    if last_header != b"" or len(first_body) != header_count + 1:
        raise AssertionError(f"TOP 0/1 header boundary differs: {headers!r} {first_body!r}")
    complete_tail = list(islice(reversed(complete_body), len(expected_body)))
    if first_returned_line != b".first" or complete_tail != list(reversed(expected_body)):
        raise AssertionError(f"TOP body line count or dot transparency differs: {first_body!r} {complete_body!r}")
    client.quit()
    print("POP3 TOP passed: zero lines, exact body count, short body and dot transparency")


def lang_states(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    """Проверяет RFC 6856 LANG в AUTHORIZATION и TRANSACTION по TLS socket."""
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    authorization_caps = client.capa()
    if "LANG" not in authorization_caps:
        raise AssertionError("LANG capability missing in AUTHORIZATION")
    _, listing, _ = client._longcmd("LANG")
    if listing != [b"en English"]:
        raise AssertionError(f"LANG listing differs: {listing!r}")
    if not client._shortcmd("LANG EN").startswith(b"+OK en "):
        raise AssertionError("case-insensitive English range was not selected")
    try:
        client._shortcmd("LANG de")
    except poplib.error_proto:
        pass
    else:
        raise AssertionError("unsupported language range unexpectedly succeeded")
    client.user(account)
    client.pass_(password)
    transaction_caps = client.capa()
    if "LANG" not in transaction_caps:
        raise AssertionError("LANG capability missing in TRANSACTION")
    if not client._shortcmd("LANG *").startswith(b"+OK en "):
        raise AssertionError("server-preferred language was not selected")
    try:
        client._shortcmd("LANG de")
    except poplib.error_proto:
        pass
    else:
        raise AssertionError("unsupported transaction language unexpectedly succeeded")
    client.stat()
    client.quit()
    print("POP3 LANG passed in AUTHORIZATION and TRANSACTION; failed selection preserves session")


def main() -> None:
    options = arguments()
    tls = context(options.ca)
    match options.phase:
        case "smtp":
            smtp(options.port, tls, options.account)
        case "pop3":
            pop3(options.port, tls, options.account, options.password)
        case "empty":
            empty(options.port, tls, options.account, options.password)
        case "lease":
            lease(options.port, tls, options.account, options.password)
        case "implicit":
            implicit(options.port, tls, options.account, options.password)
        case "rset":
            rset(options.port, tls, options.account, options.password)
        case "disconnect":
            disconnect(options.port, tls, options.account, options.password)
        case "deleted-visibility":
            deleted_visibility(
                options.port,
                options.smtp_port,
                tls,
                options.account,
                options.password,
            )
        case "top":
            top_semantics(options.port, options.smtp_port, tls, options.account, options.password)
        case "lang":
            lang_states(options.port, tls, options.account, options.password)


if __name__ == "__main__":
    main()
