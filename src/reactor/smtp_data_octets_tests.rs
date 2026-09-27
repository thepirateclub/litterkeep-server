//! RFC 2045 регрессии NUL и одиночных CR/LF в SMTP DATA.
use super::{InvalidLineOctet, validate_line_octets};

/// Принимает завершённые CRLF-последовательности и обычные content octets.
#[test]
fn accepts_canonical_crlf_without_nul() {
    assert_eq!(validate_line_octets(b"Header: value\r\nbody\r\n"), Ok(()));
}

/// Отвергает NUL без смешения причины с ошибкой canonical line ending.
#[test]
fn rejects_nul_octet() {
    assert_eq!(
        validate_line_octets(b"body\0octet\r\n"),
        Err(InvalidLineOctet::Nul)
    );
}

/// Одиночные CR и LF запрещены даже при наличии корректного завершителя.
#[test]
fn rejects_bare_cr_and_lf() {
    let cases: [&[u8]; 4] = [
        b"before\rafter\r\n",
        b"before\nafter\r\n",
        b"before\r\r\n",
        b"before\n\r\n",
    ];
    for content in cases {
        assert_eq!(
            validate_line_octets(content),
            Err(InvalidLineOctet::BareCarriageReturnOrLineFeed)
        );
    }
}
