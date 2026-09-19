"""Проверяет долговечные RENAME/DELETE и непереиспользование folder ID."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def expect(result: tuple[str, object], expected: str) -> None:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")

    if not options.verify_existing:
        expect(client.create("Lifecycle"), "OK")
        expect(client.create("Lifecycle/Child"), "OK")
        expect(
            client.append("Lifecycle/Child", None, None, b"Subject: old\r\n\r\nold"),
            "OK",
        )
        expect(client.rename("Lifecycle", "Lifecycle/Nested"), "NO")
        expect(client.rename("Lifecycle", "Renamed"), "OK")
        status, count = client.select("Renamed/Child")
        expect((status, count), "OK")
        if count != [b"1"]:
            raise AssertionError(f"subtree lost message after RENAME: {count!r}")
        expect(client.delete("Renamed"), "NO")
        expect(client.delete("Renamed/Child"), "OK")
        expect(client.create("Renamed/Child"), "OK")

    status, count = client.select("Renamed/Child")
    expect((status, count), "OK")
    if count != [b"0"]:
        raise AssertionError(f"deleted folder identity was reused: {count!r}")
    status, detail = client.select("Lifecycle/Child")
    expect((status, detail), "NO")
    client.logout()
    phase = "replayed" if options.verify_existing else "committed"
    print(f"IMAP folder lifecycle passed: rename/delete {phase}")


if __name__ == "__main__":
    main()
