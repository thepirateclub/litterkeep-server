//! Проверки grammar FETCH: группы атрибутов, макросы и MIME-части.

use super::*;
use crate::imap::sequence_set::MessageSet;

/// Атрибут остаётся типизированным после разбора скобочного списка.
#[test]
fn fetch_attribute_is_typed() {
    assert_eq!(
        parse("a3 FETCH 2 (BODYSTRUCTURE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: MessageSet::Explicit(SequenceSet::parse("2").unwrap(),),
            attributes: vec![FetchAttribute::BodyStructure],
            partial: None,
        }
    );
}

/// FETCH sequence-set не пропускает ошибочную открывающую скобку.
#[test]
fn fetch_rejects_unclosed_list_before_sequence_set() {
    assert_eq!(parse("a FETCH (1 FLAGS\r\n").command, Command::Unsupported);
}

/// RFC822.HEADER и RFC822.TEXT остаются различимыми legacy FETCH items.
#[test]
fn legacy_rfc822_sections_are_typed() {
    let Command::Fetch { attributes, .. } =
        parse("a FETCH 1 (RFC822.HEADER RFC822.TEXT)\r\n").command
    else {
        panic!("legacy RFC822 sections were not parsed");
    };
    assert_eq!(
        attributes,
        vec![FetchAttribute::Rfc822Header, FetchAttribute::Rfc822Text]
    );
}

/// BODY и BODYSTRUCTURE остаются отдельными fetch item names.
#[test]
fn body_and_bodystructure_items_are_typed_separately() {
    let Command::Fetch { attributes, .. } = parse("a FETCH 2 (BODY BODYSTRUCTURE)\r\n").command
    else {
        panic!("BODY data items were not parsed");
    };
    assert_eq!(
        attributes,
        vec![
            FetchAttribute::BodyStructureBasic,
            FetchAttribute::BodyStructure
        ]
    );
}

/// BODY section и диапазон partial представлены отдельными типами.
#[test]
fn body_peek_partial_is_typed() {
    assert_eq!(
        parse("a4 FETCH 1 (BODY.PEEK[2.MIME]<3.7>)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: MessageSet::Explicit(SequenceSet::parse("1").unwrap(),),
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
            partial: None,
        }
    );
}

/// BINARY, PEEK и SIZE сохраняют числовой MIME path и decoded partial.
#[test]
fn binary_fetch_items_are_typed() {
    let Command::Fetch { attributes, .. } =
        parse("a FETCH 1 (BINARY.PEEK[2.1]<3.7> BINARY[1] BINARY.SIZE[2.1])\r\n").command
    else {
        panic!("BINARY items were not parsed");
    };
    assert_eq!(
        attributes,
        vec![
            FetchAttribute::Binary {
                section: vec![2, 1],
                peek: true,
                partial: Some(Partial {
                    offset: 3,
                    count: 7,
                }),
            },
            FetchAttribute::Binary {
                section: vec![1],
                peek: false,
                partial: None,
            },
            FetchAttribute::BinarySize {
                section: vec![2, 1],
            },
        ]
    );
}

/// BINARY paths непустые, а decoded partial count является положительным.
#[test]
fn binary_rejects_empty_path_and_zero_partial_count() {
    for request in [
        "a FETCH 1 (BINARY[])",
        "a FETCH 1 (BINARY.SIZE[])",
        "a FETCH 1 (BINARY[1]<0.0>)",
    ] {
        assert!(matches!(parse(request).command, Command::Unsupported));
    }
}

/// HEADER.FIELDS grammar keeps field-name subset and BODY.PEEK semantics typed.
#[test]
fn header_fields_sections_are_typed() {
    assert_eq!(
        parse("a FETCH 1 (BODY.PEEK[HEADER.FIELDS (Subject FROM)]<2.10>)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set: MessageSet::Explicit(SequenceSet::parse("1").unwrap(),),
            attributes: vec![FetchAttribute::Body {
                peek: true,
                section: BodySection::HeaderFields {
                    path: Vec::new(),
                    names: vec!["Subject".into(), "FROM".into()],
                    excluded: false,
                },
                partial: Some(Partial {
                    offset: 2,
                    count: 10,
                }),
            }],
            partial: None,
        }
    );
}

/// HEADER.FIELDS.NOT с part path выбирает вложенное RFC822 message.
#[test]
fn nested_header_fields_not_is_typed() {
    let Command::Fetch { attributes, .. } =
        parse("a FETCH 1 (BODY[1.HEADER.FIELDS.NOT (BCC)] )\r\n").command
    else {
        panic!("nested header fields request was not parsed");
    };
    assert_eq!(
        attributes,
        vec![FetchAttribute::Body {
            peek: false,
            section: BodySection::HeaderFields {
                path: vec![1],
                names: vec!["BCC".into()],
                excluded: true,
            },
            partial: None,
        }]
    );
}

#[test]
fn nested_numeric_body_path_keeps_every_part_number() {
    let command = parse("A1 FETCH 1 BODY.PEEK[1.2]");
    assert!(matches!(
        command.command,
        Command::Fetch {
            attributes,
            ..
        } if attributes.contains(&FetchAttribute::Body {
            peek: true,
            section: BodySection::Part {
                path: vec![1, 2],
                mime_headers: false,
            },
            partial: None,
        })
    ));
}

/// UID FETCH не смешивает UID-диапазон с номерами последовательностей.
#[test]
fn uid_fetch_is_typed() {
    assert_eq!(
        parse("a5 UID FETCH 20:10 (RFC822.SIZE)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Uid,
            sequence_set: MessageSet::Explicit(SequenceSet::parse("20:10").unwrap(),),
            attributes: vec![FetchAttribute::Rfc822Size],
            partial: None,
        }
    );
}

#[test]
fn rfc9394_partial_modifier_is_typed_only_for_uid_fetch() {
    assert_eq!(
        parse("a10 UID FETCH 20:40 (UID FLAGS) (PARTIAL -1:-3)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Uid,
            sequence_set: MessageSet::Explicit(SequenceSet::parse("20:40").unwrap()),
            attributes: vec![FetchAttribute::Uid, FetchAttribute::Flags],
            partial: Some(crate::imap::partial_range::PartialRange::parse("-1:-3").unwrap()),
        }
    );
    for line in [
        "a11 FETCH 1 (UID FLAGS) (PARTIAL 1:2)\r\n",
        "a12 UID FETCH 1 (UID) (PARTIAL 0:2)\r\n",
        "a13 UID FETCH 1 (UID) (PARTIAL 1:2 EXTRA)\r\n",
    ] {
        assert_eq!(parse(line).command, Command::Unsupported);
    }
}

/// Saved-result marker is distinct from invalid numeric sequence syntax.
#[test]
fn fetch_saved_result_is_typed_for_sequence_and_uid_modes() {
    let saved = MessageSet::Saved;
    assert!(matches!(
        parse("a6 FETCH $ (UID FLAGS)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Sequence,
            sequence_set,
            ..
        } if sequence_set == saved
    ));
    assert!(matches!(
        parse("a7 UID FETCH $ (FLAGS)\r\n").command,
        Command::Fetch {
            addressing: Addressing::Uid,
            sequence_set,
            ..
        } if sequence_set == saved
    ));
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
                FetchAttribute::BodyStructureBasic,
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
