"""Кодирование IMAP modified UTF-7 для legacy compatibility acceptance."""

import base64


def modified_utf7(value: str) -> str:
    """Кодирует Unicode имя для клиента, который ещё не включил UTF8=ACCEPT."""
    output: list[str] = []
    non_ascii = bytearray()
    for character in value:
        if character.isascii() and 0x20 <= ord(character) <= 0x7E:
            if non_ascii:
                encoded = base64.b64encode(non_ascii).decode("ascii")
                output.append("&" + encoded.rstrip("=").replace("/", ",") + "-")
                non_ascii.clear()
            output.append("&-" if character == "&" else character)
        else:
            non_ascii.extend(character.encode("utf-16-be"))
    if non_ascii:
        encoded = base64.b64encode(non_ascii).decode("ascii")
        output.append("&" + encoded.rstrip("=").replace("/", ",") + "-")
    return "".join(output)
