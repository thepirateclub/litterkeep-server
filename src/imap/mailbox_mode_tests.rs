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

/// Остальные mailbox-команды используют тот же декодер без потери Unicode.
#[test]
fn mailbox_command_families_share_the_explicit_utf8_mode() {
    let mode = StringMode::Utf8Accept;
    let cases = [
        (
            "a CREATE \"日本語\"\r\n",
            Command::Create {
                mailbox: MailboxName::Named("日本語".into()),
                special_use: None,
            },
        ),
        (
            "a DELETE \"日本語\"\r\n",
            Command::Delete {
                mailbox: MailboxName::Named("日本語".into()),
            },
        ),
        (
            "a RENAME \"日本語\" \"旅行\"\r\n",
            Command::Rename {
                source: MailboxName::Named("日本語".into()),
                destination: MailboxName::Named("旅行".into()),
            },
        ),
        (
            "a SUBSCRIBE \"日本語\"\r\n",
            Command::Subscribe {
                mailbox: MailboxName::Named("日本語".into()),
            },
        ),
        (
            "a STATUS \"日本語\" (MESSAGES)\r\n",
            Command::Status {
                mailbox: MailboxName::Named("日本語".into()),
                attributes: vec![super::status::StatusAttribute::Messages],
            },
        ),
    ];
    for (wire, expected) in cases {
        assert_eq!(parse_with_mode(wire, mode).command, expected, "{wire:?}");
    }
    assert_eq!(
        parse_with_mode("a LIST \"\" \"旅行/%\"\r\n", mode).command,
        Command::List {
            pattern: super::folder::ListPattern::Wildcard("旅行/%".into()),
            returns: super::folder::ListReturnOptions::default(),
        }
    );
    assert_eq!(
        parse_with_mode("a LSUB \"日本語/旅行/\" %\r\n", mode).command,
        Command::Lsub {
            pattern: super::folder::ListPattern::Wildcard("日本語/旅行/%".into()),
        }
    );
}
