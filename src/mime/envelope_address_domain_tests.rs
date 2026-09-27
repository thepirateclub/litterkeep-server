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
        ("host@[nœde]", "[nœde]"),
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
    assert!(envelope.contains("[node] . mail]"));
    assert!(!envelope.contains(r"[node\\] . mail]"));

    let commented = to_field(r"Literal <host@[node\] . mail](note)>");
    let parsed = super::parse(vec![&commented]).expect("commented escaped domain-literal");
    let envelope = super::encode_list(&parsed).expect("commented domain-literal ENVELOPE");
    assert!(envelope.contains("[node] . mail]"));
    assert!(!envelope.contains(r"[node\\] . mail]"));

    let nul = to_field("Literal <host@[bad\0literal]>");
    assert!(super::parse(vec![&nul]).is_err());

    // Obsolete quoted-pair защищает NUL, а IMAP передаёт его только literal-ом.
    let escaped_nul = to_field(&format!("Literal <host@[bad\\{}literal]>", '\0'));
    let parsed = super::parse(vec![&escaped_nul]).expect("obsolete quoted NUL");
    let envelope = super::encode_list(&parsed).expect("literal-safe ENVELOPE");
    let domain = format!("[bad{}literal]", '\0');
    let literal = format!("{{{}}}\r\n{domain}", domain.len());
    assert!(envelope.contains(&literal));

    // RFC 5322 obs-qp расширяет dtext на NUL, CR, LF и ASCII controls.
    let quoted_controls = [
        '\0', '\u{1}', '\u{8}', '\u{11}', '\u{12}', '\u{14}', '\u{1f}', '\u{7f}', '\r', '\n',
    ];
    for control in quoted_controls {
        let source = format!("Literal <host@[left\\{control}right]>");
        let field = to_field(&source);
        let parsed = super::parse(vec![&field]).expect("obsolete quoted-pair address");
        let envelope = super::encode_list(&parsed).expect("obsolete address ENVELOPE");
        let domain = format!("[left{control}right]");
        match control {
            '\0' | '\r' | '\n' => {
                let literal = format!("{{{}}}\r\n{domain}", domain.len());
                assert!(
                    envelope.contains(&literal),
                    "missing literal for {control:?}"
                );
            }
            _ => assert!(envelope.contains(&domain), "missing dtext {control:?}"),
        }
    }

    // obs-NO-WS-CTL допустим непосредственно, без quoted-pair.
    let bare_obs_controls = [
        '\u{1}', '\u{8}', '\u{11}', '\u{12}', '\u{14}', '\u{1f}', '\u{7f}',
    ];
    for control in bare_obs_controls {
        let source = format!("Literal <host@[left{control}right]>");
        let field = to_field(&source);
        assert!(
            super::parse(vec![&field]).is_ok(),
            "rejected obs-dtext {control:?}"
        );
    }

    for control in ['\0', '\r', '\n'] {
        let source = format!("Literal <host@[left{control}right]>");
        let field = to_field(&source);
        assert!(
            super::parse(vec![&field]).is_err(),
            "accepted non-dtext {control:?}"
        );
    }
}
