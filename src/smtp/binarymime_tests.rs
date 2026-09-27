//! Отдельная регрессия RFC 3030 значения BODY=BINARYMIME.
use super::*;

/// Валидное расширенное значение отличимо от ошибки синтаксиса.
#[test]
fn parses_rfc3030_binarymime_body_mode() {
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=BINARYMIME\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: None,
            body_mode: BodyMode::BinaryMime,
            smtp_utf8: false,
        }))
    );
}
