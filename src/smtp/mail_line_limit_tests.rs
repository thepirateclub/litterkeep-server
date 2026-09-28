//! Проверки RFC-предела командной строки для расширенного MAIL FROM.
use super::*;

/// RFC 3030 и RFC 6531 добавляют раздельные и суммируемые allowances.
#[test]
fn composes_binarymime_and_smtputf8_command_line_extensions() {
    assert_eq!(MAX_COMMAND_LINE_SIZE, 512);
    assert_eq!(MAX_SMTPUTF8_MAIL_LINE_SIZE, 522);
    assert_eq!(MAX_BINARYMIME_MAIL_LINE_SIZE, 528);
    assert_eq!(MAX_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE, 538);

    assert_eq!(
        command_line_limit(b"MAIL FROM:<a@test> BODY=BINARYMIME\r\n"),
        528
    );
    assert_eq!(
        command_line_limit(b"MAIL FROM:<a@test> BODY=BINARYMIME SMTPUTF8\r\n"),
        538
    );
    assert_eq!(
        command_line_limit(b"mail from:<a@test> body=binarymime smtputf8\r\n"),
        538
    );
}

/// RFC 1870 SIZE allowance складывается с SMTPUTF8 и BINARYMIME.
#[test]
fn composes_size_with_other_mail_line_extensions() {
    assert_eq!(MAX_SIZE_MAIL_LINE_SIZE, 538);
    assert_eq!(MAX_SIZE_SMTPUTF8_MAIL_LINE_SIZE, 548);
    assert_eq!(MAX_SIZE_BINARYMIME_MAIL_LINE_SIZE, 554);
    assert_eq!(MAX_SIZE_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE, 564);

    for (parameters, expected) in [
        ("SIZE=1", MAX_SIZE_MAIL_LINE_SIZE),
        ("size=123", MAX_SIZE_MAIL_LINE_SIZE),
        ("SIZE=1 SMTPUTF8", MAX_SIZE_SMTPUTF8_MAIL_LINE_SIZE),
        ("SIZE=1 BODY=BINARYMIME", MAX_SIZE_BINARYMIME_MAIL_LINE_SIZE),
        (
            "SIZE=1 BODY=BINARYMIME SMTPUTF8",
            MAX_SIZE_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE,
        ),
    ] {
        let line = format!("MAIL FROM:<a@test> {parameters}\r\n");
        assert_eq!(command_line_limit(line.as_bytes()), expected, "{line}");
    }
}

/// RFC 1870 allowance нужен на реальной границе command receiver-а.
#[test]
fn accepts_mail_line_through_size_extension_boundary() {
    let prefix = "MAIL FROM:<a@test> SIZE=123 ";
    let padding = "X".repeat(MAX_SIZE_MAIL_LINE_SIZE - prefix.len() - 2);
    let line = format!("{prefix}{padding}\r\n");

    assert_eq!(line.len(), MAX_SIZE_MAIL_LINE_SIZE);
    assert_eq!(command_line_limit(line.as_bytes()), MAX_SIZE_MAIL_LINE_SIZE);
}
