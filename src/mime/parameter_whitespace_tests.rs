//! RFC 2045/2231 syntax checks for MIME parameter separators.

use super::parameters;

/// OWS после `;` разрешён, а пробелы возле обязательного `=` нет.
#[test]
fn requires_adjacent_equals_for_regular_and_extended_parameters() {
    for value in [
        "application/octet-stream; name =x",
        "application/octet-stream; name= x",
        "application/octet-stream; filename* =UTF-8''file.txt",
        "application/octet-stream; filename*= UTF-8''file.txt",
        "application/octet-stream; filename*0 *=UTF-8''part",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
    assert!(parameters::parse("application/octet-stream; name=x").is_ok());
    assert!(parameters::parse("application/octet-stream;  name=x").is_ok());
    assert!(parameters::parse("application/octet-stream;\tname=x").is_ok());
    assert!(parameters::parse("application/octet-stream; name(comment)=x").is_ok());
    assert!(parameters::parse("application/octet-stream; name=(comment)x").is_ok());
}
