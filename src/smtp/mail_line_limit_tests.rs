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
