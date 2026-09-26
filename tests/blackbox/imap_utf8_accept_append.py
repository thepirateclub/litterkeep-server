"""Проверяет RFC 9755 APPEND до и после ENABLE независимым imaplib-клиентом."""

import argparse
import imaplib
import ssl
from pathlib import Path


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    return parser.parse_args()


def account_password(path: Path, account: str) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        identity, separator, password = line.partition("\t")
        if separator and identity == account and password:
            return password
    raise AssertionError(f"acceptance credential for {account} is missing")


def require_empty_inbox(client: imaplib.IMAP4) -> None:
    status, counts = client.select("INBOX", readonly=True)
    if status != "OK" or counts != [b"0"]:
        raise AssertionError(f"rejected APPEND became visible: {status!r} {counts!r}")


def authenticated_client(options: argparse.Namespace, password: str) -> imaplib.IMAP4:
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port, timeout=10)
    client.starttls(ssl_context=context)
    client.login("bob", password)
    return client


def require_bad_create(client: imaplib.IMAP4, mailbox: bytes, reason: str) -> None:
    """Проверяет tagged BAD для сырого имени и восстановление сессии."""
    tag = client._command("CREATE", mailbox)
    status, response = client._get_tagged_response(tag)
    if status != "BAD":
        raise AssertionError(f"{reason}: expected BAD, received {status!r} {response!r}")
    client.noop()


def append_utf8_header(
    client: imaplib.IMAP4,
    mailbox: str,
    message: bytes,
) -> tuple[str, list[bytes]]:
    status, response = client.append(mailbox, None, None, message)
    if status != "OK":
        raise AssertionError(f"UTF8=ACCEPT APPEND failed: {response!r}")
    return status, response


def main() -> None:
    options = arguments()
    password = account_password(Path(options.credentials), "bob")
    client = authenticated_client(options, password)
    status, capabilities = client.capability()
    if status != "OK" or any(b"UTF8=ACCEPT" in item.upper() for item in capabilities):
        raise AssertionError(f"partial extension was advertised: {capabilities!r}")

    legacy_tag = client._command("CREATE", '"旅行"'.encode("utf-8"))
    legacy_status, legacy_data = client._get_tagged_response(legacy_tag)
    if legacy_status != "BAD":
        raise AssertionError(f"legacy UTF-8 CREATE was not rejected: {legacy_status!r} {legacy_data!r}")
    client.noop()

    message = "From: bob@litterkeep.test\r\nSubject: café\r\n\r\nbody\x00bytes".encode()
    status, rejected = client.append("INBOX", None, None, message)
    if status != "NO" or not any(b"UTF-8 headers require ENABLE" in item for item in rejected):
        raise AssertionError(f"pre-ENABLE APPEND was not rejected: {status!r} {rejected!r}")
    require_empty_inbox(client)
    client.logout()

    # RFC 5161 требует выполнить ENABLE в AUTH state, до SELECT/EXAMINE.
    client = authenticated_client(options, password)
    # CAPABILITY скрыта до прохождения всего acceptance profile, поэтому
    # отправляем ENABLE через низкоуровневый независимый imaplib command path.
    status, _ = client._simple_command("ENABLE", "UTF8=ACCEPT")
    response_type, enabled = client.response("ENABLED")
    if status != "OK" or response_type != "ENABLED" or not any(
        b"UTF8=ACCEPT" in item for item in enabled
    ):
        raise AssertionError(f"ENABLE did not report UTF8=ACCEPT: {status!r} {enabled!r}")
    client._encoding = "utf-8"
    require_bad_create(client, b'"\xc3("', "malformed UTF-8 quoted mailbox")
    require_bad_create(client, b'"\xc2\x85"', "forbidden C1 control in mailbox")
    require_bad_create(client, b'"\x01"', "forbidden C0 control in mailbox")
    require_bad_create(client, "旅行".encode("utf-8"), "UTF-8 mailbox atom")
    status, created = client.create("旅行")
    if status != "OK":
        raise AssertionError(f"UTF-8 CREATE failed: {created!r}")
    status, subscribed = client.subscribe("旅行")
    if status != "OK":
        raise AssertionError(f"UTF-8 SUBSCRIBE failed: {subscribed!r}")
    status, folders = client.list("", "*")
    if status != "OK" or not any('"旅行"'.encode("utf-8") in item for item in folders):
        raise AssertionError(f"UTF-8 LIST response missing mailbox: {folders!r}")
    status, subscribed_folders = client.lsub("", "*")
    if status != "OK" or not any(
        '"旅行"'.encode("utf-8") in item for item in subscribed_folders
    ):
        raise AssertionError(f"UTF-8 LSUB response missing mailbox: {subscribed_folders!r}")
    status, mailbox_status = client.status("旅行", "(MESSAGES)")
    if status != "OK" or not any('"旅行"'.encode("utf-8") in item for item in mailbox_status):
        raise AssertionError(f"UTF-8 STATUS response missing mailbox: {mailbox_status!r}")
    status, counts = client.select("旅行", readonly=True)
    if status != "OK" or counts != [b"0"]:
        raise AssertionError(f"UTF-8 SELECT failed: {status!r} {counts!r}")

    # imaplib._mode_utf8 добавляет в APPEND устаревший data item UTF8 из
    # RFC 6855; RFC 9755 удалил его, поэтому оставляем современную framing.
    status, appended = append_utf8_header(client, "旅行", message)
    if not any(b"APPENDUID 1 1" in item for item in appended):
        raise AssertionError(f"accepted APPENDUID missing: {appended!r}")
    status, counts = client.select("旅行", readonly=True)
    if status != "OK" or counts != [b"1"]:
        raise AssertionError(f"accepted APPEND count mismatch: {status!r} {counts!r}")
    status, fetched = client.fetch("1", "(RFC822)")
    stored = next(
        value
        for item in fetched
        if isinstance(item, tuple)
        for value in item
        if isinstance(value, bytes) and value.startswith(b"From:")
    )
    if status != "OK" or stored != message:
        raise AssertionError("accepted UTF-8 APPEND bytes changed")
    client.logout()
    print(
        "RFC 9755 mailbox and APPEND transition passed: "
        f"CREATE/LIST/LSUB/STATUS/SELECT, octets={len(message)}"
    )


if __name__ == "__main__":
    main()
