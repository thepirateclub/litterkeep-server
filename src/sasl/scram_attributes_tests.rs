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
fn accepts_large_opaque_fields_and_bounds_attribute_count() {
    let large_nonce = format!("r={}", "A".repeat(2 * 1024 * 1024));
    assert_eq!(
        parse(&large_nonce)
            .expect("large nonce")
            .first()
            .map(|item| item.value.len()),
        Some(2 * 1024 * 1024)
    );
    let many = std::iter::repeat_n("x=value", MAX_ATTRIBUTE_COUNT + 1)
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(parse(&many), Err(ParseError::TooManyAttributes));
}

#[test]
fn client_final_accepts_large_transcripts_without_a_profile_cap() {
    let transcript = format!("x={}", "a".repeat(2 * 1024 * 1024));
    assert!(parse_client_final(&transcript).is_ok());
}

#[test]
fn accepts_utf8_and_space_in_opaque_values_but_rejects_nul() {
    for valid in ["n=has space", "n=юзер", "n=tab\tvalue"] {
        assert!(parse(valid).is_ok(), "rejected {valid:?}");
    }
    assert!(parse("n=has\0nul").is_err());
}
