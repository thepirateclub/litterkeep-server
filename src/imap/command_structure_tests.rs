use super::{Command, MailboxName, TaggedCommand, parse};

/// Структурная скобка не должна пропускаться при выборе command verb.
#[test]
fn command_verb_cannot_follow_a_list_delimiter() {
    assert_eq!(
        parse("A1 (NOOP)\r\n"),
        TaggedCommand {
            tag: "A1".into(),
            command: Command::Unsupported,
        }
    );
}

/// Quoted-string не является atom в production command и UID subcommand.
#[test]
fn quoted_command_verbs_are_rejected() {
    assert_eq!(parse("A2 \"NOOP\"\r\n").command, Command::Unsupported);
    assert_eq!(parse("A3 UID \"FETCH\"\r\n").command, Command::Unsupported);
}

/// Пустой обязательный UID sequence-set не перескакивает через list boundary.
#[test]
fn uid_expunge_requires_an_atom_sequence_set() {
    assert_eq!(parse("A4 UID EXPUNGE ()\r\n").command, Command::Unsupported);
}

/// LOGIN допускает resp-special, но не atom-special внутри некавычечного astring.
#[test]
fn login_astring_rejects_unquoted_specials_and_accepts_bracket() {
    for value in ["bo%ob", "bo*b", "bo{b", "bo\\b"] {
        assert_eq!(
            parse(&format!("A5 LOGIN {value} password\r\n")).command,
            Command::Unsupported
        );
    }
    assert_eq!(
        parse("A6 LOGIN bo]b password\r\n").command,
        Command::Login {
            username: "bo]b".into(),
            password: b"password".to_vec(),
        }
    );
    assert_eq!(
        parse("A7 LOGIN \"bo%ob\" password\r\n").command,
        Command::Login {
            username: "bo%ob".into(),
            password: b"password".to_vec(),
        }
    );
    assert_eq!(
        parse("A8 LOGIN bob password extra\r\n").command,
        Command::Unsupported
    );
}

/// SELECT также требует astring и допускает atom-special только в quoted form.
#[test]
fn select_mailbox_requires_valid_astring_form() {
    assert_eq!(
        parse("A9 SELECT folder%name\r\n").command,
        Command::Unsupported
    );
    assert_eq!(
        parse("A10 SELECT \"folder%name\"\r\n").command,
        Command::Select {
            mailbox: MailboxName::Named("folder%name".into()),
        }
    );
}
