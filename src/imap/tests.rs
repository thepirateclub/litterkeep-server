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

/// FETCH attribute остаётся типизированным после lexer-а списков.
#[test]
fn fetch_attribute_is_typed() {
    assert_eq!(
        parse("a3 FETCH 2 (BODYSTRUCTURE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: SequenceSet::parse("2").unwrap(),
            attribute: FetchAttribute::BodyStructure,
        }
    );
}

/// BODY section и partial измеряются отдельными предметными типами.
#[test]
fn body_peek_partial_is_typed() {
    assert_eq!(
        parse("a4 FETCH 1 (BODY.PEEK[2.MIME]<3.7>)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: SequenceSet::parse("1").unwrap(),
            attribute: FetchAttribute::Body {
                peek: true,
                section: BodySection::Part {
                    path: vec![2],
                    mime_headers: true,
                },
                partial: Some(Partial {
                    offset: 3,
                    count: 7
                }),
            },
        }
    );
}

/// UID FETCH сохраняет отдельное пространство адресации до store snapshot.
#[test]
fn uid_fetch_is_typed() {
    assert_eq!(
        parse("a5 UID FETCH 20:10 (RFC822.SIZE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Uid,
            sequence_set: SequenceSet::parse("20:10").unwrap(),
            attribute: FetchAttribute::Rfc822Size,
        }
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
        }
    );
}

/// STORE operation и системные flags не покидают parser строками.
#[test]
fn store_flags_are_typed() {
    let Command::Store {
        addressing,
        sequence_set,
        mutation,
        silent,
        flags,
    } = parse("a8 UID STORE 4:9 +FLAGS.SILENT (\\Seen \\Flagged)\r\n").command
    else {
        panic!("STORE was not parsed");
    };
    assert_eq!(addressing, Addressing::Uid);
    assert_eq!(sequence_set, SequenceSet::parse("4:9").unwrap());
    assert_eq!(mutation, crate::mail_flags::FlagMutation::Add);
    assert!(silent);
    assert!(flags.contains(crate::mail_flags::SystemFlag::Seen));
    assert!(flags.contains(crate::mail_flags::SystemFlag::Flagged));
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
