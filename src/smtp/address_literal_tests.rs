//! RFC-тесты самостоятельной SMTP address-literal грамматики.
use super::*;

#[test]
fn accepts_ipv4_snums_and_registered_ipv6_tag() {
    for domain in [
        "[192.0.2.1]",
        "[001.002.003.004]",
        "[IPv6:2001:db8::1]",
        "[ipv6:::ffff:192.0.2.1]",
        "[IPv6:::ffff:192.000.2.1]",
    ] {
        assert_eq!(parse_domain(domain), LiteralForm::Valid, "{domain}");
    }
}

#[test]
fn rejects_invalid_octets_unregistered_tags_and_bad_ipv6() {
    for domain in [
        "[]",
        "[256.0.0.1]",
        "[0000.0.0.1]",
        "[192.0.2]",
        "[192.0.2.1.4]",
        "[IPv6:2001:db8:::1]",
        "[IPv7:2001:db8::1]",
        "[IPv6:2001:db8::1",
    ] {
        assert_eq!(parse_domain(domain), LiteralForm::Invalid, "{domain}");
    }
}
