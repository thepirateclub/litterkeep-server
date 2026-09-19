"""Проверяет documented conflict: IMAP expunge побеждает POP3 QUIT snapshot."""

import argparse
import imaplib
import poplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--pop3-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def main() -> None:
    options = arguments()
    tls = ssl.create_default_context(cafile=options.ca)
    imap = imaplib.IMAP4("127.0.0.1", options.imap_port)
    imap.starttls(ssl_context=tls)
    imap.login("pop", "pop-password")
    imap.append("INBOX", None, None, b"Subject: conflict\r\n\r\nbody")
    pop = poplib.POP3("127.0.0.1", options.pop3_port)
    pop.stls(context=tls)
    pop.user("pop")
    pop.pass_("pop-password")
    imap.select("INBOX")
    imap.store("1", "+FLAGS", "(\\Deleted)")
    imap.expunge()
    _, lines, _ = pop.retr(1)
    if b"Subject: conflict" not in b"\r\n".join(lines):
        raise AssertionError("POP3 snapshot lost immutable message bytes")
    pop.dele(1)
    try:
        pop.quit()
    except poplib.error_proto as error:
        if "delete commit failed" not in str(error):
            raise
    else:
        raise AssertionError("POP3 promised deletion already done by IMAP")
    _, count = imap.select("INBOX")
    if count != [b"0"]:
        raise AssertionError(f"IMAP expunge did not persist: {count!r}")
    imap.logout()
    print("POP3/IMAP conflict passed: snapshot readable, IMAP wins, QUIT reports failure")


if __name__ == "__main__":
    main()
