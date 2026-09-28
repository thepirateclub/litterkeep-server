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
fn client_final_allows_nonce_and_binding_overhead_but_remains_bounded() {
    let exact = format!("x={}", "a".repeat(MAX_CLIENT_FINAL_SIZE - 2));
    let oversized = format!("{exact}a");

    assert_eq!(exact.len(), MAX_CLIENT_FINAL_SIZE);
    assert!(parse_client_final(&exact).is_ok());
    assert_eq!(
        parse_client_final(&oversized),
        Err(ParseError::MessageTooLarge)
    );
    assert_eq!(parse(&exact), Err(ParseError::MessageTooLarge));
}

#[test]
fn accepts_utf8_and_space_in_opaque_values_but_rejects_nul() {
    for valid in ["n=has space", "n=юзер", "n=tab\tvalue"] {
        assert!(parse(valid).is_ok(), "rejected {valid:?}");
    }
    assert!(parse("n=has\0nul").is_err());
}
