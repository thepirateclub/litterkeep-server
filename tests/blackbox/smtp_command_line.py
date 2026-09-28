"""Проверяет базовый RFC 5321 предел SMTP-командных строк."""

import os
import smtplib
import ssl


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))


def expect_reply(actual, expected, context):
    code, response = actual
    if code != expected:
        raise AssertionError(
            f"{context}: expected {expected}, received {code} {response!r}"
        )


def probe_mail_line(client, prefix, wire_size, expected, context):
    """Проверяет, какой слой первым обрабатывает заданную длину MAIL."""
    padding_size = wire_size - len(prefix) - 2
    if padding_size < 0:
        raise ValueError(f"invalid MAIL boundary size: {wire_size}")
    line = prefix + b"X" * padding_size + b"\r\n"
    if len(line) != wire_size:
        raise AssertionError(f"constructed MAIL line has wrong size: {len(line)}")
    client.send(line)
    expect_reply(client.getreply(), expected, context)
    expect_reply(client.noop(), 250, f"session after {context}")


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP(HOST, PORT, timeout=10) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")

        client.send(b"NOOP " + b"x" * 505 + b"\r\n")
        expect_reply(client.getreply(), 250, "512-octet command line")
        client.send(b"NOOP " + b"x" * 506 + b"\r\n")
        expect_reply(client.getreply(), 500, "513-octet command line")
        expect_reply(client.noop(), 250, "command channel after rejected line")

        probe_mail_line(
            client,
            b"MAIL FROM:<a@test> SIZE=1 X=",
            538,
            555,
            "RFC 1870 538-octet SIZE MAIL line",
        )
        probe_mail_line(
            client,
            b"MAIL FROM:<a@test> SIZE=1 X=",
            539,
            500,
            "RFC 1870 539-octet SIZE MAIL line",
        )
        probe_mail_line(
            client,
            b"MAIL FROM:<a@test> size=1 body=binarymime smtputf8 X=",
            564,
            555,
            "combined 564-octet MAIL line",
        )
        probe_mail_line(
            client,
            b"MAIL FROM:<a@test> size=1 body=binarymime smtputf8 X=",
            565,
            500,
            "combined 565-octet MAIL line",
        )

    print(
        "PASS smtp-command-line: 512/513 base, 538/539 SIZE, "
        "564/565 combined boundaries and session recovery"
    )


if __name__ == "__main__":
    main()
