use super::*;

/// Проверяет передачу реквизитов в типизированном варианте.
#[test]
fn login_has_credentials() {
    assert_eq!(
        parse("a LOGIN bob pass\r\n"),
        TaggedCommand {
            tag: "a".into(),
            command: Command::Login {
                username: "bob".into(),
                password: "pass".into()
            }
        }
    );
}

/// STARTTLS преобразуется в enum до входа в session FSM.
#[test]
fn starttls_is_typed() {
    assert_eq!(
        parse("a2 STARTTLS\r\n"),
        TaggedCommand {
            tag: "a2".into(),
            command: Command::StartTls,
        }
    );
}

/// NAMESPACE и UNSELECT принимают только пустую аргументную форму.
#[test]
fn namespace_and_unselect_commands_are_typed() {
    assert_eq!(parse("a NAMESPACE\r\n").command, Command::Namespace);
    assert_eq!(parse("b UNSELECT\r\n").command, Command::Unselect);
    assert_eq!(parse("c NAMESPACE extra\r\n").command, Command::Unsupported);
    assert_eq!(parse("d UNSELECT extra\r\n").command, Command::Unsupported);
}

/// Команды без аргументов отвергают даже один лишний atom как синтаксическую ошибку.
#[test]
fn no_argument_commands_reject_trailing_arguments() {
    for line in [
        "a CAPABILITY extra\r\n",
        "b LOGOUT extra\r\n",
        "c NOOP extra\r\n",
        "d IDLE extra\r\n",
        "e STARTTLS extra\r\n",
    ] {
        assert_eq!(parse(line).command, Command::Unsupported, "{line}");
    }
}

/// Capability atoms ENABLE становятся типизированными Enum до передачи reactor-у.
#[test]
fn enable_capability_atoms_are_typed() {
    assert_eq!(
        parse("a ENABLE IMAP4rev2 UTF8=ACCEPT X-UNKNOWN\r\n").command,
        Command::Enable {
            capabilities: vec![
                enable::Capability::Imap4rev2,
                enable::Capability::Utf8Accept,
                enable::Capability::Unsupported,
            ],
        }
    );
    assert_eq!(parse("b ENABLE\r\n").command, Command::Unsupported);
    assert_eq!(
        parse("b ENABLE \"IMAP4rev2\"\r\n").command,
        Command::Unsupported
    );
    assert_eq!(
        parse("c ENABLE (IMAP4rev2)\r\n").command,
        Command::Unsupported
    );
}

/// Специальное имя INBOX нормализуется без строкового сравнения в reactor.
#[test]
fn inbox_is_typed_case_insensitively() {
    assert_eq!(
        parse("a6 EXAMINE inbox\r\n").command,
        Command::Examine {
            mailbox: MailboxName::Inbox,
        }
    );
}

/// APPEND отделяет объявленный размер literal от последующих сетевых байтов.
#[test]
fn append_literal_size_is_typed() {
    assert_eq!(
        parse("a7 APPEND INBOX {42}\r\n").command,
        Command::Append {
            mailbox: MailboxName::Inbox,
            literal_size: 42,
            flags: crate::mail_flags::MessageFlags::default(),
            keywords: std::collections::BTreeSet::new(),
            internal_date: None,
        }
    );
}

#[test]
fn append_metadata_is_typed() {
    let Command::Append {
        literal_size,
        flags,
        keywords,
        internal_date,
        ..
    } = parse("a8 APPEND INBOX (\\Seen \\Draft) \"19-Sep-2026 12:34:56 +0300\" {9}\r\n").command
    else {
        panic!("APPEND metadata was not parsed");
    };
    assert_eq!(literal_size, 9);
    assert!(flags.contains(crate::mail_flags::SystemFlag::Seen));
    assert!(flags.contains(crate::mail_flags::SystemFlag::Draft));
    assert!(keywords.is_empty());
    assert_eq!(
        internal_date.expect("date").encode(),
        "19-Sep-2026 09:34:56 +0000"
    );
}

#[test]
fn create_and_list_are_typed() {
    assert_eq!(
        parse("a10 CREATE \"Projects/2026\"\r\n").command,
        Command::Create {
            mailbox: MailboxName::Named("Projects/2026".into()),
            special_use: None,
        }
    );
    assert_eq!(
        parse("a11 LIST \"\" \"%\"\r\n").command,
        Command::List {
            pattern: crate::imap::folder::ListPattern::OneLevel,
            returns: crate::imap::folder::ListReturnOptions::default(),
        }
    );
}

/// Mailbox parser принимает RFC 3501 modified UTF-7 и хранит Unicode вне wire.
#[test]
fn mailbox_name_decodes_modified_utf7() {
    assert_eq!(parse("a UTF7 &ZeVnLIqe-\r\n").command, Command::Unsupported);
    assert_eq!(
        parse("a CREATE &ZeVnLIqe-\r\n").command,
        Command::Create {
            mailbox: MailboxName::Named("日本語".into()),
            special_use: None,
        }
    );
}

#[test]
fn delete_and_recursive_rename_are_typed() {
    assert_eq!(
        parse("a12 DELETE \"Projects/2025\"\r\n").command,
        Command::Delete {
            mailbox: MailboxName::Named("Projects/2025".into())
        }
    );
    assert_eq!(
        parse("a13 RENAME Projects Work\r\n").command,
        Command::Rename {
            source: MailboxName::Named("Projects".into()),
            destination: MailboxName::Named("Work".into())
        }
    );
}

#[test]
fn subscriptions_and_status_are_typed() {
    assert_eq!(
        parse("a14 SUBSCRIBE \"Projects/2026\"\r\n").command,
        Command::Subscribe {
            mailbox: MailboxName::Named("Projects/2026".into())
        }
    );
    assert_eq!(
        parse("a15 LSUB \"\" *\r\n").command,
        Command::Lsub {
            pattern: crate::imap::folder::ListPattern::All
        }
    );
    assert_eq!(
        parse("a16 STATUS INBOX (MESSAGES UIDNEXT UIDVALIDITY UNSEEN)\r\n").command,
        Command::Status {
            mailbox: MailboxName::Inbox,
            attributes: vec![
                crate::imap::status::StatusAttribute::Messages,
                crate::imap::status::StatusAttribute::UidNext,
                crate::imap::status::StatusAttribute::UidValidity,
                crate::imap::status::StatusAttribute::Unseen,
            ]
        }
    );
    assert_eq!(
        parse("a17 STATUS INBOX (RECENT)\r\n").command,
        Command::Status {
            mailbox: MailboxName::Inbox,
            attributes: vec![crate::imap::status::StatusAttribute::Recent]
        }
    );
}

#[test]
fn expunge_requires_no_arguments() {
    assert_eq!(parse("a18 EXPUNGE\r\n").command, Command::Expunge);
    assert_eq!(parse("a19 EXPUNGE now\r\n").command, Command::Unsupported);
}

#[test]
fn uid_expunge_requires_non_empty_uid_set() {
    assert!(matches!(
        parse("a UID EXPUNGE 2:4\r\n").command,
        Command::UidExpunge { .. }
    ));
    assert_eq!(parse("a UID EXPUNGE\r\n").command, Command::Unsupported);
}

#[test]
fn close_and_check_require_no_arguments() {
    assert_eq!(parse("a CLOSE\r\n").command, Command::Close);
    assert_eq!(parse("a CHECK\r\n").command, Command::Check);
    assert_eq!(parse("a CLOSE later\r\n").command, Command::Unsupported);
    assert_eq!(parse("a CHECK later\r\n").command, Command::Unsupported);
}

#[test]
fn search_uses_typed_key_and_addressing() {
    assert_eq!(
        parse("a SEARCH UNSEEN\r\n").command,
        Command::Search {
            addressing: Addressing::Sequence,
            key: crate::imap::search::SearchKey::Unseen,
            charset: crate::imap::search_charset::Charset::Utf8,
            result: crate::imap::search_result::ResultRequest::Legacy,
        }
    );
    assert_eq!(
        parse("a UID SEARCH DELETED\r\n").command,
        Command::Search {
            addressing: Addressing::Uid,
            key: crate::imap::search::SearchKey::Deleted,
            charset: crate::imap::search_charset::Charset::Utf8,
            result: crate::imap::search_result::ResultRequest::Legacy,
        }
    );
    assert!(matches!(
        parse("a SEARCH SUBJECT x\r\n").command,
        Command::Search { .. }
    ));
}

/// DONE разбирается как continuation, а не как tagged команда.
#[test]
fn idle_done_is_typed() {
    assert!(matches!(
        parse_idle_continuation("done\r\n"),
        IdleContinuation::Done
    ));
}

#[test]
fn noop_is_typed() {
    assert_eq!(parse("A8 NOOP\r\n").command, Command::Noop);
}
