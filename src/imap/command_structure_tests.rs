use super::{Command, TaggedCommand, parse};

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
