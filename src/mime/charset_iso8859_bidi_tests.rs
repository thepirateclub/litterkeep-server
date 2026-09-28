//! Проверяет RFC 1556 bidi charset profiles и их IANA aliases.

use super::{Charset, Charset::*};

#[test]
fn directional_labels_remain_distinct_typed_profiles() {
    for (label, expected) in [
        ("ISO-8859-6-E", Iso8859_6Explicit),
        ("ISO_8859-6-E", Iso8859_6Explicit),
        ("csISO88596E", Iso8859_6Explicit),
        ("ISO-8859-6-I", Iso8859_6Implicit),
        ("ISO_8859-6-I", Iso8859_6Implicit),
        ("csISO88596I", Iso8859_6Implicit),
        ("ISO-8859-8-E", Iso8859_8Explicit),
        ("ISO_8859-8-E", Iso8859_8Explicit),
        ("csISO88598E", Iso8859_8Explicit),
        ("ISO-8859-8-I", Iso8859_8Implicit),
        ("ISO_8859-8-I", Iso8859_8Implicit),
        ("csISO88598I", Iso8859_8Implicit),
    ] {
        assert_eq!(Charset::parse(label), Ok(expected), "{label}");
    }
}

#[test]
fn arabic_and_hebrew_variants_share_only_their_octet_mapping() {
    for charset in [Iso8859_6Explicit, Iso8859_6Implicit] {
        assert_eq!(
            charset.decode(b"\xc7\xe4\xc7\xd3\xe4\xc7\xe5"),
            Ok("الاسلام".to_owned())
        );
        assert!(charset.decode(b"\xa1").is_err());
    }
    for charset in [Iso8859_8Explicit, Iso8859_8Implicit] {
        assert_eq!(charset.decode(b"\xf9\xec\xe5\xed"), Ok("שלום".to_owned()));
        assert_eq!(
            charset.decode(b"\xfd\xfe"),
            Ok("\u{200e}\u{200f}".to_owned())
        );
        assert!(charset.decode(b"\xfb").is_err());
    }
}
