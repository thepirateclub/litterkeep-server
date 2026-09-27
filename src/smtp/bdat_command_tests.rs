//! RFC 3030 chunk-size и LAST grammar regressions.

use super::{super::BdatCommand, parse};
use crate::smtp::ParseError;

/// Проверяет нулевой chunk, ведущие нули и нечувствительный регистр LAST.
#[test]
fn parses_zero_intermediate_and_last_chunks() {
    let cases = [
        (
            "0",
            BdatCommand {
                chunk_size: 0,
                last: false,
            },
        ),
        (
            "0007",
            BdatCommand {
                chunk_size: 7,
                last: false,
            },
        ),
        (
            "12 LAST",
            BdatCommand {
                chunk_size: 12,
                last: true,
            },
        ),
        (
            "12 last",
            BdatCommand {
                chunk_size: 12,
                last: true,
            },
        ),
    ];
    for (wire, expected) in cases {
        assert_eq!(parse(wire), Ok(expected), "BDAT argument: {wire}");
    }
}

/// Отвергает пустой размер, знаки, лишние аргументы и переполнение u64.
#[test]
fn rejects_malformed_chunk_arguments() {
    let cases = [
        "",
        "-1",
        "+1",
        "1\tLAST",
        "1  LAST",
        "1 LAST extra",
        "18446744073709551616",
    ];
    for wire in cases {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "BDAT argument: {wire}"
        );
    }
}
