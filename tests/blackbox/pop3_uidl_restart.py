"""Проверяет POP3 UIDL через SMTP-доставку и перезапуск Linux-сервера."""

import argparse
import base64
from email.message import EmailMessage
from enum import Enum
import json
import poplib
from pathlib import Path
import smtplib
import ssl


class FixtureAccount(Enum):
    """Имена стандартных acceptance-аккаунтов."""

    ALICE = "alice"
    BOB = "bob"


class AcceptancePhase(Enum):
    """Этапы, разделённые фактическим перезапуском сервера."""

    SEED = "seed"
    VERIFY = "verify"


def read_credentials(path: Path) -> dict[FixtureAccount, str]:
    """Загружает acceptance-секреты из файла, не печатая их и не передавая в argv."""
    credentials: dict[FixtureAccount, str] = {}
    for line in path.read_text(encoding="ascii").splitlines():
        fields = line.split("\t")
        match fields:
            case [name, password]:
                try:
                    account = FixtureAccount(name)
                except ValueError:
                    continue
                credentials[account] = password
            case _:
                raise ValueError(f"Некорректная строка credentials: {path}")
    if FixtureAccount.ALICE not in credentials:
        raise ValueError("В credentials отсутствует Alice")
    if FixtureAccount.BOB not in credentials:
        raise ValueError("В credentials отсутствует Bob")
    return credentials


def parse_uidl(lines: list[bytes]) -> list[str]:
    """Проверяет UIDL listing и возвращает только семантические UID."""
    unique_ids: list[str] = []
    expected_number = 1
    for line in lines:
        number, separator, encoded_uid = line.partition(b" ")
        if separator != b" ":
            raise AssertionError(f"Неверная UIDL строка: {line!r}")
        if number != str(expected_number).encode("ascii"):
            raise AssertionError(f"Неверный POP3 номер в UIDL listing: {line!r}")
        if not 1 <= len(encoded_uid) <= 70:
            raise AssertionError(f"Длина UIDL вне RFC 1939: {encoded_uid!r}")
        if not all(33 <= octet <= 126 for octet in encoded_uid):
            raise AssertionError(f"UIDL содержит недопустимый октет: {encoded_uid!r}")
        unique_ids.append(encoded_uid.decode("ascii"))
        expected_number += 1
    if len(set(unique_ids)) != len(unique_ids):
        raise AssertionError("UIDL повторяется внутри maildrop")
    return unique_ids


def deliver(port: int, context: ssl.SSLContext, credentials: dict[FixtureAccount, str], subject: str) -> None:
    """Доставляет отдельное письмо Alice через submission SMTP STARTTLS."""
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = subject
    message.set_content(subject)
    with smtplib.SMTP("localhost", port, timeout=15) as client:
        client.starttls(context=context)
        client.login(FixtureAccount.ALICE.value, credentials[FixtureAccount.ALICE])
        rejected = client.send_message(message)
    if rejected:
        raise AssertionError(f"SMTP отклонил адресата: {rejected!r}")


def authenticated_pop3(
    port: int,
    context: ssl.SSLContext,
    credentials: dict[FixtureAccount, str],
) -> poplib.POP3:
    """Открывает Bob maildrop только после успешного STLS и PASS."""
    client = poplib.POP3("localhost", port, timeout=15)
    client.stls(context=context)
    payload = b"\x00" + FixtureAccount.BOB.value.encode("ascii")
    payload += b"\x00" + credentials[FixtureAccount.BOB].encode("ascii")
    encoded = base64.b64encode(payload).decode("ascii")
    response = client._shortcmd(f"AUTH PLAIN {encoded}")
    if not response.startswith(b"+OK"):
        raise AssertionError(f"SASL PLAIN отклонён после STLS: {response!r}")
    return client


def seed(options: argparse.Namespace, context: ssl.SSLContext, credentials: dict[FixtureAccount, str]) -> None:
    """Создаёт два сообщения и фиксирует исходный POP3 UIDL listing."""
    existing = authenticated_pop3(options.pop3_port, context, credentials)
    count, _ = existing.stat()
    existing.quit()
    match count:
        case 0:
            deliver(options.smtp_port, context, credentials, "UIDL restart first")
            deliver(options.smtp_port, context, credentials, "UIDL restart second")
        case 1:
            deliver(options.smtp_port, context, credentials, "UIDL restart second")
        case 2:
            pass
        case _:
            raise AssertionError(f"Seed fixture уже содержит {count} сообщений")
    client = authenticated_pop3(options.pop3_port, context, credentials)
    count, _ = client.stat()
    _, lines, _ = client.uidl()
    identifiers = parse_uidl(lines)
    client.close()
    if count != 2:
        raise AssertionError(f"Ожидались два письма перед рестартом: {count}")
    if len(identifiers) != 2:
        raise AssertionError(f"Ожидались два письма перед рестартом: {count}, {identifiers!r}")
    options.state.write_text(json.dumps(identifiers), encoding="ascii")


def verify_restart(
    options: argparse.Namespace,
    context: ssl.SSLContext,
    credentials: dict[FixtureAccount, str],
) -> None:
    """Проверяет сохранение UIDL, durable удаления и неповторный новый UID."""
    original = json.loads(options.state.read_text(encoding="ascii"))
    if not isinstance(original, list):
        raise ValueError(f"Список UIDL ожидался в {options.state}")
    if len(original) != 2:
        raise ValueError(f"Некорректное состояние UIDL: {options.state}")
    client = authenticated_pop3(options.pop3_port, context, credentials)
    _, lines, _ = client.uidl()
    recovered = parse_uidl(lines)
    if recovered != original:
        raise AssertionError(f"UIDL изменились после рестарта: {original!r} -> {recovered!r}")
    client.dele(1)
    client.quit()

    deliver(options.smtp_port, context, credentials, "UIDL restart third")
    replay = authenticated_pop3(options.pop3_port, context, credentials)
    count, _ = replay.stat()
    _, lines, _ = replay.uidl()
    current = parse_uidl(lines)
    replay.quit()
    retained, created = current
    _, expected_retained = original
    if retained != expected_retained:
        raise AssertionError("UIDL оставшегося второго письма изменился")
    if created in original:
        raise AssertionError("новое письмо повторно использовало существующий UIDL")
    if count != 2:
        raise AssertionError(f"Ожидались два письма после доставки: {count}")


def arguments() -> argparse.Namespace:
    """Разбирает wire endpoints и локальные fixture paths."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--phase",
        choices=tuple(phase.value for phase in AcceptancePhase),
        required=True,
    )
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--pop3-port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    """Запускает одну из двух фаз вокруг внешнего рестарта сервера."""
    options = arguments()
    phase = AcceptancePhase(options.phase)
    context = ssl.create_default_context(cafile=options.ca)
    credentials = read_credentials(options.credentials)
    match phase:
        case AcceptancePhase.SEED:
            seed(options, context, credentials)
        case AcceptancePhase.VERIFY:
            verify_restart(options, context, credentials)
    print(f"PASS POP3 UIDL {phase.value}: restart-stable and unique")


if __name__ == "__main__":
    main()
