//! Parser acceptance для SELECT/EXAMINE с явно выбранным string mode.

use super::tokenizer::StringMode;
use super::{Command, MailboxName, parse, parse_with_mode};

/// SELECT остаётся legacy по умолчанию и принимает UTF-8 только явно в mode.
#[test]
fn select_mailbox_uses_explicit_utf8_accept_mode() {
    let wire = "a SELECT \"日本語/Invoices\"\r\n";
    assert_eq!(
        parse(wire).command,
        Command::Unsupported,
        "production parser remains in legacy mode"
    );
    assert_eq!(
        parse_with_mode(wire, StringMode::Utf8Accept).command,
        Command::Select {
            mailbox: MailboxName::Named("日本語/Invoices".into())
        }
    );
}

/// EXAMINE использует ту же валидацию и отклоняет запрещённые Unicode controls.
#[test]
fn examine_rejects_forbidden_utf8_mailbox_scalars() {
    assert_eq!(
        parse_with_mode(
            "a EXAMINE \"Invoices\u{2028}2026\"\r\n",
            StringMode::Utf8Accept
        )
        .command,
        Command::Unsupported
    );
}
