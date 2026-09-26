//! Тесты форм EHLO/HELO из RFC 5321 §4.1.1.1.
use super::super::{Command, ParseError, parse};

#[test]
fn accepts_domain_for_both_greetings() {
    assert_eq!(
        parse("EHLO sender.test\r\n"),
        Ok(Command::Ehlo("sender.test".into()))
    );
    assert_eq!(
        parse("HELO sender.test\r\n"),
        Ok(Command::Helo("sender.test".into()))
    );
}

#[test]
fn accepts_address_literal_only_for_ehlo() {
    assert_eq!(
        parse("EHLO [192.0.2.1]\r\n"),
        Ok(Command::Ehlo("[192.0.2.1]".into()))
    );
    assert_eq!(
        parse("EHLO [IPv6:2001:db8::1]\r\n"),
        Ok(Command::Ehlo("[IPv6:2001:db8::1]".into()))
    );
    assert_eq!(parse("HELO [192.0.2.1]\r\n"), Err(ParseError::Syntax));
}

#[test]
fn rejects_malformed_or_multiple_greeting_arguments() {
    for wire in [
        "EHLO\r\n",
        "HELO\r\n",
        "EHLO sender.test another.test\r\n",
        "HELO sender.test another.test\r\n",
        "EHLO sender..test\r\n",
        "HELO -sender.test\r\n",
        "EHLO sender.test \r\n",
        "EHLO\tsender.test\r\n",
    ] {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "wire command: {wire:?}"
        );
    }
}
