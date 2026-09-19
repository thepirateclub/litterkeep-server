import argparse
import imaplib
import ssl

parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, required=True)
parser.add_argument("--ca", required=True)
args = parser.parse_args()
client = imaplib.IMAP4("127.0.0.1", args.port)
client.starttls(ssl_context=ssl.create_default_context(cafile=args.ca))
client.login("bob", "bob-password")
tag = client._new_tag()
client.send(tag + b" CREATE Archive2 (USE (\\Archive))\r\n")
status, _ = client._get_tagged_response(tag)
if status != "OK": raise AssertionError("CREATE failed")
status, data = client.list("", "Archive2")
if status != "OK" or not any(b"\\Archive" in item for item in data if isinstance(item, bytes)):
    raise AssertionError(f"missing special-use: {data!r}")
client.logout()
print("IMAP CREATE SPECIAL-USE passed")
