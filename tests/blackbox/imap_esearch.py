"""Проверяет ESEARCH correlator, UID и result options по STARTTLS."""

import argparse
import imaplib
import os
import ssl
import uuid


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2144)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    arguments = parser.parse_args()
    context = ssl.create_default_context(cafile=arguments.ca)
    client = imaplib.IMAP4(arguments.host, arguments.port, timeout=10)
    client.starttls(ssl_context=context)
    password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(client.login("bob", password), "OK")
    mailbox = f"Esearch-{uuid.uuid4().hex}"
    require(client.create(mailbox), "OK")
    for flags, subject in [
        ("(\\Seen)", "one"),
        (None, "two"),
        (None, "three"),
    ]:
        require(
            client.append(
                mailbox,
                flags,
                None,
                f"Subject: {subject}\r\n\r\nbody\r\n".encode("ascii"),
            ),
            "OK",
        )
    require(client.select(mailbox), "OK")
    status, data = client.search(None, "RETURN", "(COUNT MIN MAX ALL)", "ALL")
    require((status, data), "OK")
    response = client.response("ESEARCH")
    text = b" ".join(response[1] or [])
    assert b'(TAG "' in text, text
    assert b"COUNT 3" in text, text
    assert b"MIN 1" in text, text
    assert b"MAX 3" in text, text
    assert b"ALL 1:3" in text, text
    status, data = client.uid("SEARCH", "RETURN", "(COUNT MIN MAX ALL)", "ALL")
    require((status, data), "OK")
    response = client.response("ESEARCH")
    text = b" ".join(response[1] or [])
    assert b" UID " in text, text
    assert b"ALL 1:3" in text, text
    status, data = client.search(None, "SEEN")
    assert status == "OK" and data == [b"1"], (status, data)
    status, data = client.search(None, "RETURN", "()", "SEEN")
    require((status, data), "OK")
    response = client.response("ESEARCH")
    assert b"ALL 1" in b" ".join(response[1] or []), response
    status, data = client.search(None, "RETURN", "(ALL)", "SUBJECT", '"missing"')
    require((status, data), "OK")
    response = client.response("ESEARCH")
    empty_result = b" ".join(response[1] or [])
    assert b"ALL" not in empty_result, response
    assert b"COUNT" not in empty_result, response
    assert b"MIN" not in empty_result, response
    assert b"MAX" not in empty_result, response
    status, data = client.search(None, "RETURN", "(COUNT MIN MAX ALL)", "SUBJECT", '"missing"')
    require((status, data), "OK")
    response = client.response("ESEARCH")
    empty_result = b" ".join(response[1] or [])
    assert b"COUNT 0" in empty_result, response
    assert b"ALL" not in empty_result, response
    assert b"MIN" not in empty_result, response
    assert b"MAX" not in empty_result, response
    status, data = client.search(None, "RETURN", "()", "SUBJECT", '"missing"')
    require((status, data), "OK")
    response = client.response("ESEARCH")
    assert b"ALL" not in b" ".join(response[1] or []), response
    client.logout()
    print("PASS imap-esearch")


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


if __name__ == "__main__":
    main()
