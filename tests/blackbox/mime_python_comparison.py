"""Сравнивает наш MIME decoder с независимым parser стандартной библиотеки."""

import argparse
import email.header
import email.message
import email.policy
from email.parser import BytesParser
import subprocess


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("probe")
    return parser.parse_args()


def python_subject() -> bytes:
    decoded = email.header.decode_header("=?UTF-8?B?0J/RgNC40LLQtdGC?=")
    parts = []
    for value, charset in decoded:
        match value:
            case bytes():
                parts.append(value.decode(charset or "ascii"))
            case str():
                parts.append(value)
    return "".join(parts).encode("utf-8")


def python_filename() -> bytes:
    message = email.message.Message()
    message["Content-Disposition"] = (
        "attachment; filename*0*=UTF-8''%D1%81%D1%87; "
        "filename*1*=%D1%91%D1%82.txt"
    )
    filename = message.get_filename()
    if filename is None:
        raise AssertionError("Python parser did not decode filename")
    return filename.encode("utf-8")


def main() -> None:
    result = subprocess.run(
        [arguments().probe],
        check=True,
        capture_output=True,
    )
    lines = iter(result.stdout.splitlines())
    actual_subject = next(lines)
    actual_filename = next(lines)
    actual_count = int(next(lines))
    actual_body = next(lines)
    actual_composite = next(lines)
    actual_nested_body = next(lines)
    if actual_subject != python_subject():
        raise AssertionError("encoded-word result differs from Python")
    if actual_filename != python_filename():
        raise AssertionError("RFC 2231 filename differs from Python")
    multipart = (
        b"Content-Type: multipart/mixed; boundary=x\r\n\r\npreamble\r\n"
        b"--x\r\nContent-Type: text/plain\r\n\r\ninside --x text\r\n"
        b"--x--\r\nepilogue"
    )
    parsed = BytesParser(policy=email.policy.default).parsebytes(multipart)
    python_parts = tuple(parsed.iter_parts())
    python_body = next(iter(python_parts)).get_payload(decode=True)
    if actual_count != len(python_parts):
        raise AssertionError("multipart part count differs from Python")
    if actual_body != python_body:
        raise AssertionError("multipart body range differs from Python")
    composite = "Re: =?UTF-8?B?0J/RgNC40LLQtdGC?= =?ISO-8859-1?Q?_caf=E9?="
    composite_parts = email.header.decode_header(composite)
    decoded_composite = "".join(
        value.decode(charset or "ascii") if isinstance(value, bytes) else value
        for value, charset in composite_parts
    ).encode("utf-8")
    if actual_composite != decoded_composite:
        raise AssertionError("composite encoded-word differs from Python")
    nested_raw = (
        b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\n"
        b"Content-Type: text/plain\r\n\r\ninner body"
    )
    nested_message = BytesParser(policy=email.policy.default).parsebytes(nested_raw)
    nested_payload = next(iter(nested_message.iter_parts())).get_payload(decode=True)
    if actual_nested_body != nested_payload:
        raise AssertionError("message/rfc822 body range differs from Python")
    print("MIME Python comparison passed")


if __name__ == "__main__":
    main()
