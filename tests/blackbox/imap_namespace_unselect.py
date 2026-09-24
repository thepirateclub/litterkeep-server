"""Независимая проверка NAMESPACE и безопасной IMAP UNSELECT команды."""
import argparse
import imaplib
import os
import ssl


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
OPTIONS = PARSER.parse_args()
BOB_PASSWORD = os.environ.get("LITTERKEEP_BOB_PASSWORD", "test-password")
TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE

with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
    status, capabilities = imap.capability()
    assert status == "OK", (status, capabilities)
    capability_atoms = b" ".join(capabilities).upper()
    assert b"NAMESPACE" in capability_atoms, capabilities
    assert b"UNSELECT" in capability_atoms, capabilities
    imap.starttls(ssl_context=TLS_CONTEXT)
    status, _ = imap.login("bob", BOB_PASSWORD)
    assert status == "OK", status

    status, namespace = imap.namespace()
    assert status == "OK", (status, namespace)
    namespace_bytes = b" ".join(namespace)
    assert b'(("" "/")) NIL NIL' in namespace_bytes, namespace

    status, count = imap.select("INBOX")
    assert status == "OK", status
    status, matches = imap.search(None, "HEADER", "SUBJECT", '"body section acceptance"')
    assert status == "OK", (status, matches)
    sequence = next(iter(next(iter(matches)).decode("ascii").split()))
    status, stored = imap.store(sequence, "+FLAGS", "(\\Deleted)")
    assert status == "OK", (status, stored)

    status, _ = imap._simple_command("UNSELECT")
    assert status == "OK", status
    status, _ = imap.select("INBOX")
    assert status == "OK", status
    status, deleted = imap.search(None, "DELETED")
    assert status == "OK", (status, deleted)
    assert sequence.encode("ascii") in next(iter(deleted)), deleted

    status, restored = imap.store(sequence, "-FLAGS", "(\\Deleted)")
    assert status == "OK", (status, restored)

print("PASS imap-namespace-unselect")
