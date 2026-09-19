"""Проверяет event-driven IDLE двумя независимыми IMAP-сессиями."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def connect(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    return client


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    waiting = connect(options.port, context)
    producer = connect(options.port, context)
    if "IDLE" not in waiting.capabilities:
        raise AssertionError("server did not advertise implemented IDLE")
    status, counts = waiting.select("INBOX")
    if status != "OK" or counts != [b"0"]:
        raise AssertionError(f"unexpected initial mailbox: {counts!r}")
    raw = b"From: bob@litterkeep.test\r\nSubject: IDLE event\r\n\r\nbody"
    with waiting.idle(duration=5) as idler:
        status, _ = producer.append("INBOX", None, None, raw)
        if status != "OK":
            raise AssertionError("producer APPEND failed")
        response_type, response_data = next(idler)
        if response_type != "EXISTS" or response_data != [b"1"]:
            raise AssertionError(
                f"unexpected IDLE notification: {response_type!r} {response_data!r}"
            )
    status, _ = waiting.noop()
    if status != "OK":
        raise AssertionError("session did not resume after DONE")
    waiting.logout()
    producer.logout()
    print("IMAP IDLE passed: durable APPEND -> EXISTS -> DONE")


if __name__ == "__main__":
    main()
