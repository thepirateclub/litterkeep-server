//! Проверки grammar FETCH: группы атрибутов, макросы и MIME-части.

use super::*;

/// Атрибут остаётся типизированным после разбора скобочного списка.
#[test]
fn fetch_attribute_is_typed() {
    assert_eq!(
        parse("a3 FETCH 2 (BODYSTRUCTURE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: SequenceSet::parse("2").unwrap(),
            attributes: vec![FetchAttribute::BodyStructure],
        }
    );
}

/// BODY section и диапазон partial представлены отдельными типами.
#[test]
fn body_peek_partial_is_typed() {
    assert_eq!(
        parse("a4 FETCH 1 (BODY.PEEK[2.MIME]<3.7>)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: SequenceSet::parse("1").unwrap(),
            attributes: vec![FetchAttribute::Body {
                peek: true,
                section: BodySection::Part {
                    path: vec![2],
                    mime_headers: true,
                },
                partial: Some(Partial {
                    offset: 3,
                    count: 7,
                }),
            }],
        }
    );
}

/// UID FETCH не смешивает UID-диапазон с номерами последовательностей.
#[test]
fn uid_fetch_is_typed() {
    assert_eq!(
        parse("a5 UID FETCH 20:10 (RFC822.SIZE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Uid,
            sequence_set: SequenceSet::parse("20:10").unwrap(),
            attributes: vec![FetchAttribute::Rfc822Size],
        }
    );
}

/// Сервер принимает несколько data item и сохраняет порядок запроса.
#[test]
fn fetch_parses_multiple_data_items_and_uid() {
    let Command::Fetch { attributes, .. } =
        parse("a FETCH 1 (UID FLAGS RFC822.SIZE INTERNALDATE)\r\n").command
    else {
        panic!("FETCH data item list was not parsed");
    };
    assert_eq!(
        attributes,
        vec![
            FetchAttribute::Uid,
            FetchAttribute::Flags,
            FetchAttribute::Rfc822Size,
            FetchAttribute::InternalDate,
        ]
    );
}

/// Каждый FETCH macro раскрывается ровно в набор, заданный протоколом.
#[test]
fn fetch_macros_expand_to_their_protocol_data_items() {
    let cases = [
        (
            "a FETCH 1 ALL\r\n",
            vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
                FetchAttribute::Envelope,
            ],
        ),
        (
            "a FETCH 1 FAST\r\n",
            vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
            ],
        ),
        (
            "a FETCH 1 FULL\r\n",
            vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
                FetchAttribute::Envelope,
                FetchAttribute::BodyStructure,
            ],
        ),
    ];
    for (wire, expected) in cases {
        let Command::Fetch { attributes, .. } = parse(wire).command else {
            panic!("FETCH macro was not parsed: {wire}");
        };
        assert_eq!(attributes, expected, "wrong FETCH macro expansion: {wire}");
    }
}

/// INTERNALDATE остаётся typed атрибутом и без скобочного набора.
#[test]
fn internal_date_fetch_is_typed() {
    let Command::Fetch { attributes, .. } = parse("a9 FETCH 1 INTERNALDATE\r\n").command else {
        panic!("FETCH not parsed");
    };
    assert_eq!(attributes, vec![FetchAttribute::InternalDate]);
}
