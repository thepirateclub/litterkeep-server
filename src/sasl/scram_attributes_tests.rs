//! Регрессии границ грамматики атрибутов SCRAM.
use super::*;

#[test]
fn parses_known_fields_and_preserves_base64_padding() {
    let parsed = parse("c=biws,r=clientNonce,p=AA==").expect("valid client-final fields");
    assert_eq!(
        parsed,
        vec![
            Attribute {
                name: AttributeName::ChannelBinding,
                value: "biws".to_owned(),
            },
            Attribute {
                name: AttributeName::Nonce,
                value: "clientNonce".to_owned(),
            },
            Attribute {
                name: AttributeName::Proof,
                value: "AA==".to_owned(),
            },
        ]
    );
}

#[test]
fn rejects_empty_missing_and_multicharacter_attributes() {
    for invalid in ["", "n", "=value", "nn=value", "ю=value", "n=", "n=a,,r=b"] {
        assert!(parse(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn rejects_duplicate_fields_and_mandatory_extensions() {
    assert_eq!(parse("r=one,r=two"), Err(ParseError::DuplicateAttribute));
    assert_eq!(
        parse("m=unknown,n=user"),
        Err(ParseError::MandatoryExtensionUnsupported)
    );
}

#[test]
fn bounds_message_and_attribute_count_before_unbounded_collection() {
    let oversized = "x".repeat(MAX_MESSAGE_SIZE + 1);
    assert_eq!(parse(&oversized), Err(ParseError::MessageTooLarge));
    let many = std::iter::repeat_n("x=value", MAX_ATTRIBUTE_COUNT + 1)
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(parse(&many), Err(ParseError::TooManyAttributes));
}

#[test]
fn rejects_non_ascii_and_whitespace_attribute_values() {
    for invalid in ["n=has space", "n=has\ttab", "n=юзер"] {
        assert!(parse(invalid).is_err(), "accepted {invalid:?}");
    }
}
