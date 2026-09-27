//! Регрессии разделителей внутри RFC 5322 domain-literal.

use crate::mime::header_block::HeaderField;

fn to_field(value: &str) -> HeaderField {
    HeaderField {
        name: "to".to_owned(),
        value: value.as_bytes().to_vec(),
        raw_range: 0..value.len(),
    }
}

/// Символы `@` и `>` внутри литерала не меняют границы angle-addr.
#[test]
fn preserves_domain_literal_separators_inside_angle_address() {
    for (address, expected_domain) in [
        ("host@[mail@node]", "[mail@node]"),
        ("host@[node>mail]", "[node>mail]"),
        ("host@[mail(box)@node]", "[mail(box)@node]"),
        ("host@[mail(box@node]", "[mail(box@node]"),
    ] {
        let field = to_field(&format!("Literal <{address}>"));
        let parsed = super::parse(vec![&field]).expect("domain-literal address");
        let envelope = super::encode_list(&parsed).expect("domain-literal ENVELOPE");
        assert!(envelope.contains(expected_domain));
    }

    let malformed = to_field("Literal <host@node]>");
    assert!(super::parse(vec![&malformed]).is_err());

    let escaped_closer = to_field(r"Literal <host@[node\]mail]>");
    assert!(super::parse(vec![&escaped_closer]).is_ok());

    let escaped_dot = to_field(r"Literal <host@[node\] . mail]>");
    let parsed = super::parse(vec![&escaped_dot]).expect("escaped domain-literal byte");
    let envelope = super::encode_list(&parsed).expect("escaped domain-literal ENVELOPE");
    assert!(envelope.contains(r"[node\\] . mail]"));
}
