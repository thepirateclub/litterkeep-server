"""Проверяет, что TLS не подменяет недоверие CA и неверный пароль успехом."""
import imaplib
import socket
import ssl
import sys


def arguments(values):
    host = next(values, None)
    port_text = next(values, None)
    ca_path = next(values, None)
    account = next(values, None)
    extra = next(values, None)
    match (host, port_text, ca_path, account, extra):
        case (str(), str(), str(), str(), None):
            return host, int(port_text), ca_path, account
        case _:
            raise SystemExit("usage: tls_imap_rejections.py HOST PORT CA_PEM ACCOUNT")


def rejects_unknown_ca(host: str, port: int) -> None:
    context = ssl.create_default_context()
    try:
        with socket.create_connection((host, port), timeout=3) as connection:
            context.wrap_socket(connection, server_hostname=host)
    except ssl.SSLCertVerificationError:
        return
    raise AssertionError("unknown CA was accepted")


def rejects_wrong_password(host: str, port: int, ca_path: str, account: str) -> None:
    context = ssl.create_default_context(cafile=ca_path)
    client = imaplib.IMAP4_SSL(host, port, ssl_context=context)
    try:
        try:
            client.login(account, "wrong-password")
        except imaplib.IMAP4.error:
            return
        raise AssertionError("wrong password was accepted")
    finally:
        client.logout()


def main() -> None:
    values = iter(sys.argv)
    next(values, None)
    host, port, ca_path, account = arguments(values)
    rejects_unknown_ca(host, port)
    rejects_wrong_password(host, port, ca_path, account)
    print("untrusted CA and wrong password were rejected")


if __name__ == "__main__":
    main()
