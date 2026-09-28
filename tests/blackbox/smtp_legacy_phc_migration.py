"""Независимый SMTP TLS acceptance миграции legacy Argon2 credentials."""

import argparse
from enum import IntEnum, StrEnum
import os
from pathlib import Path
import smtplib
import ssl


class AccountName(StrEnum):
    """Аккаунт, для которого acceptance преобразует PHC fixture."""

    ALICE = "alice"


class SmtpReply(IntEnum):
    """SMTP-код успешной SASL PLAIN аутентификации."""

    AUTHENTICATED = 235


def arguments() -> argparse.Namespace:
    """Читает параметры отдельного listener-а и fixture data directory."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("data_dir", type=Path)
    return parser.parse_args()


def alice_password(data_dir: Path) -> str:
    """Находит пароль Alice в bootstrap fixture без позиционных индексов."""
    credentials = data_dir.joinpath("test-accounts.tsv").read_text(encoding="utf-8")
    for line in credentials.splitlines():
        fields = line.split("\t")
        match fields:
            case [account, password]:
                match AccountName(account):
                    case AccountName.ALICE:
                        return password
                    case _:
                        continue
            case _:
                raise AssertionError("bootstrap credential row is malformed")
    raise AssertionError("Alice credential is absent from bootstrap fixture")


def prepare_legacy_record(data_dir: Path) -> tuple[Path, bytes]:
    """Перемещает bootstrap PHC2 в legacy-слот в одноразовом acceptance store."""
    account = AccountName.ALICE.value
    account_dir = data_dir.joinpath("accounts")
    normalized = account_dir.joinpath(f"{account}.phc2")
    legacy = account_dir.joinpath(f"{account}.phc")
    if not normalized.is_file():
        raise AssertionError("bootstrap did not create Alice PHC2 credential")
    if legacy.exists():
        raise AssertionError("legacy fixture path already exists")
    original = normalized.read_bytes()
    os.link(normalized, legacy)
    normalized.unlink()
    return legacy, original


def authenticate(host: str, port: int, context: ssl.SSLContext, password: str) -> None:
    """Подтверждает AUTH PLAIN по wire через стандартный SMTP TLS клиент."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        code, _reply = client.login(AccountName.ALICE.value, password)
        match SmtpReply(code):
            case SmtpReply.AUTHENTICATED:
                return
            case _:
                raise AssertionError(f"unexpected SMTP AUTH reply: {code}")


def prove_new_record_priority(
    host: str,
    port: int,
    context: ssl.SSLContext,
    password: str,
    legacy: Path,
    original: bytes,
) -> None:
    """Временно убирает fallback и проверяет вход только по опубликованному PHC2."""
    backup = legacy.with_name("alice.phc.acceptance-backup")
    legacy.rename(backup)
    try:
        authenticate(host, port, context, password)
    finally:
        backup.rename(legacy)
    if legacy.read_bytes() != original:
        raise AssertionError("legacy credential bytes changed during migration")


def main() -> None:
    """Проверяет legacy login, durable PHC2 publication и повторную проверку."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    password = alice_password(options.data_dir)
    legacy, original = prepare_legacy_record(options.data_dir)
    authenticate(options.host, options.port, context, password)
    normalized = options.data_dir.joinpath("accounts", "alice.phc2")
    if not normalized.is_file():
        raise AssertionError("legacy login did not publish normalized PHC2")
    if legacy.read_bytes() != original:
        raise AssertionError("legacy credential bytes changed during migration")
    prove_new_record_priority(
        options.host, options.port, context, password, legacy, original
    )
    print("PASS SMTP TLS legacy PHC migration and PHC2-only authentication")


if __name__ == "__main__":
    main()
