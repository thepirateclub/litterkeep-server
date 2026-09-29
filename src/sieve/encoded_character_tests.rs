//! RFC 5228 encoded-character vectors и capability gating.

use super::{decode, is_required};
use crate::sieve::ast::Command;
use crate::sieve::lexer::{Token, tokenize_octets};

#[test]
fn decodes_hex_octets_and_unicode_scalars_case_insensitively() {
    assert_eq!(decode(b"${hex: 40 0a FF}").expect("hex"), b"@\n\xff");
    assert_eq!(
        decode(b"${UNICODE: 1f600 41}").expect("unicode sequence"),
        "😀A".as_bytes()
    );
}

#[test]
fn preserves_malformed_sequences_and_decodes_nested_sequences() {
    assert_eq!(
        decode(b"${hex:400}").expect("malformed is literal"),
        b"${hex:400}"
    );
    assert_eq!(
        decode(b"${hex:4${hex:30}}").expect("nested sequence"),
        b"${hex:40}"
    );
    assert_eq!(decode(b"$${hex:24 24}").expect("literal marker"), b"$$$");
}

#[test]
fn rejects_well_formed_unicode_values_outside_scalar_range() {
    assert!(decode(b"${unicode:D800}").is_err());
    assert!(decode(b"${unicode:200000}").is_err());
    assert_eq!(
        decode(b"${Unicode:Cool}").expect("malformed is literal"),
        b"${Unicode:Cool}"
    );
    assert_eq!(
        decode(b"${unicode:FFFFFFFFFFFFFFFFG}").expect("malformed overflow is literal"),
        b"${unicode:FFFFFFFFFFFFFFFFG}"
    );
}

#[test]
fn activation_requires_case_sensitive_declared_capability() {
    let encoded = tokenize_octets(b"require \"encoded-character\"; keep;").expect("tokens");
    let upper = tokenize_octets(b"require \"ENCODED-CHARACTER\"; keep;").expect("tokens");
    let later = tokenize_octets(b"keep; require \"encoded-character\";").expect("tokens");
    assert!(is_required(&encoded));
    assert!(!is_required(&upper));
    assert!(!is_required(&later));
}

#[test]
fn syntax_token_is_available_for_validation() {
    assert_eq!(
        tokenize_octets(b"require \"encoded-character\";").expect("tokens"),
        vec![
            Token::Atom("require".into()),
            Token::String(b"encoded-character".to_vec()),
            Token::Semicolon
        ]
    );
}

#[test]
fn decoded_hex_string_reaches_the_byte_valued_ast() {
    let script = crate::sieve::parse(
        "require \"encoded-character\"; if header :is \"Subject\" \"${hex:FF 80}\" { keep; }",
    )
    .expect("encoded-character script");
    let Some(Command::If {
        condition: crate::sieve::Condition::Header { matcher, .. },
        ..
    }) = script.commands.first()
    else {
        panic!("string condition expected")
    };
    assert_eq!(matcher.keys, vec![vec![0xff, 0x80]]);
}

#[test]
fn encoded_character_remains_literal_without_the_extension() {
    let script = crate::sieve::parse("if header :is \"Subject\" \"${hex:41}\" { keep; }")
        .expect("base Sieve script");
    let Some(Command::If {
        condition: crate::sieve::Condition::Header { matcher, .. },
        ..
    }) = script.commands.first()
    else {
        panic!("string condition expected")
    };
    assert_eq!(matcher.keys, vec![b"${hex:41}".to_vec()]);
}

#[test]
fn encoded_character_runs_after_quoted_string_unescaping() {
    let script = crate::sieve::parse(
        "require \"encoded-character\"; if header :is \"Subject\" \"$${hex:24 24}\" { keep; }",
    )
    .expect("quoted encoded-character");
    let Some(Command::If {
        condition: crate::sieve::Condition::Header { matcher, .. },
        ..
    }) = script.commands.first()
    else {
        panic!("header condition expected")
    };
    assert_eq!(matcher.keys, vec![b"$$$".to_vec()]);
}
