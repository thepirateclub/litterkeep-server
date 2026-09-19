"""Проверяет extended LIST RETURN (SPECIAL-USE) независимым IMAP-клиентом."""
import argparse
import imaplib
import ssl


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    args = parser.parse_args()
    client = imaplib.IMAP4("127.0.0.1", args.port)
    client.starttls(ssl_context=ssl.create_default_context(cafile=args.ca))
    if client.login("bob", "bob-password")[0] != "OK":
        raise AssertionError("LOGIN failed")
    tag = client._new_tag()
    client.send(tag + b' LIST "" "*" RETURN (SPECIAL-USE)\r\n')
    status, detail = client._get_tagged_response(tag)
    if status != "OK":
        raise AssertionError(f"LIST RETURN failed: {detail!r}")
    values = client.untagged_responses.pop("LIST", [])
    if not any(b"\\Sent" in value for value in values if isinstance(value, bytes)):
        raise AssertionError(f"special-use attribute missing: {values!r}")
    client.logout()
    print("IMAP LIST RETURN SPECIAL-USE passed")


if __name__ == "__main__":
    main()
