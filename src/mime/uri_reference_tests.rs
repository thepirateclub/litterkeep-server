//! Табличные проверки URI-reference grammar и границы MIME projection.

use super::is_valid;

#[test]
fn accepts_rfc3986_absolute_and_relative_references() {
    for value in [
        "g:h",
        "g",
        "./g",
        "g/",
        "/g",
        "//g",
        "?y",
        "g?y",
        "#s",
        "g#s",
        "g?y#s",
        ";x",
        "g;x",
        "g;x?y#s",
        "http://a/b/c/d;p?q",
        "https://user:pass@[2001:db8::1]:443/a%20b",
        "../files/a%20b.bin",
        "http://[v1.a:b]/",
        "file:///path",
        "",
    ] {
        assert!(is_valid(value), "expected valid URI-reference: {value:?}");
    }
}

#[test]
fn rejects_malformed_components_and_authorities() {
    for value in [
        "%",
        "%2",
        "a b",
        "путь",
        "1a:b",
        "http://2001:db8::1/",
        "http://[2001:db8::zz]/",
        "http://[v1.a%20b]/",
        "http://[2001:db8::1",
        "http://host:port/path",
        "http://a@b@c/path",
        "http://host/%zz",
        "http://host/[path]",
        "relative|path",
    ] {
        assert!(
            !is_valid(value),
            "expected invalid URI-reference: {value:?}"
        );
    }
}
