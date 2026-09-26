//! Проверки точной границы RFC 5321 между командой и SMTP path.
use super::{Command, ParseError, parse};

/// RFC 5321 не разрешает пробел между двоеточием и SMTP path.
#[test]
fn rejects_whitespace_between_path_command_colon_and_angle_bracket() {
    for wire in [
        "MAIL FROM: <alice@test>\r\n",
        "MAIL FROM:\t<alice@test>\r\n",
        "RCPT TO: <bob@test>\r\n",
        "RCPT TO:\t<bob@test>\r\n",
    ] {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "wire command: {wire:?}"
        );
    }
    assert!(matches!(
        parse("MAIL FROM:<alice@test> SIZE=1\r\n"),
        Ok(Command::Mail(_))
    ));
    assert_eq!(
        parse("RCPT TO:<bob@test>\r\n"),
        Ok(Command::Rcpt("bob@test".into()))
    );
}

/// RFC 5321 ESMTP parameters разделяются одиночными ASCII-пробелами.
#[test]
fn rejects_tabs_repeated_spaces_and_trailing_space_after_path() {
    for wire in [
        "MAIL FROM:<alice@test>\tSIZE=1\r\n",
        "MAIL FROM:<alice@test>  SIZE=1\r\n",
        "MAIL FROM:<alice@test> SIZE=1 \r\n",
        "RCPT TO:<bob@test> \r\n",
        "RCPT TO:<bob@test>\tNOTIFY=SUCCESS\r\n",
    ] {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "wire command: {wire:?}"
        );
    }
    assert!(matches!(
        parse("MAIL FROM:<alice@test> SIZE=1 BODY=7BIT\r\n"),
        Ok(Command::Mail(_))
    ));
}
