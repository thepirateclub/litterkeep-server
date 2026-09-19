"""Независимо проверяет trusted implicit-TLS IMAP и вход с паролем.

Клиент использует только стандартную библиотеку Python. Он проверяет цепочку
сертификата и IP SAN, поэтому не принимает сертификат через trust-all режим.
"""
import imaplib
import ssl
import sys


def required(values) -> tuple[str, int, str, str, str]:
    host = next(values, None)
    port_text = next(values, None)
    ca_path = next(values, None)
    account = next(values, None)
    password = next(values, None)
    extra = next(values, None)
    match (host, port_text, ca_path, account, password, extra):
        case (str(), str(), str(), str(), str(), None):
            return host, int(port_text), ca_path, account, password
        case _:
            raise SystemExit("usage: tls_imap_login.py HOST PORT CA_PEM ACCOUNT PASSWORD")


def main() -> None:
    values = iter(sys.argv)
    next(values, None)
    host, port, ca_path, account, password = required(values)
    context = ssl.create_default_context(cafile=ca_path)
    client = imaplib.IMAP4_SSL(host, port, ssl_context=context)
    try:
        status, _ = client.login(account, password)
        assert status == "OK", status
        print("trusted TLS IMAP login passed")
    finally:
        client.logout()


if __name__ == "__main__":
    main()
