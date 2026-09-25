//! RFC 5322 group and mailbox fixtures for the IMAP ENVELOPE projection.

use super::*;
use crate::mime::header_block::HeaderField;

fn to_field(value: &str) -> HeaderField {
    HeaderField {
        name: "to".to_owned(),
        value: value.as_bytes().to_vec(),
        raw_range: 0..value.len(),
    }
}

/// Группа сохраняется маркерами RFC 9051, а её элементы остаются адресами.
#[test]
fn encodes_group_markers_and_members_in_wire_order() {
    let field =
        to_field("\"A Group, East\":Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;");
    let parsed = parse(vec![&field]).expect("group address-list");
    let encoded = encode_list(&parsed).expect("IMAP address-list");
    assert_eq!(
        encoded,
        "((NIL NIL \"A Group, East\" NIL)(\"Ed Jones\" NIL \"c\" \"a.test\")(NIL NIL \"joe\" \"where.test\")(\"John\" NIL \"jdoe\" \"one.test\")(NIL NIL NIL NIL))"
    );
}

/// Пустая RFC 5322 group не исчезает из ENVELOPE и имеет обе границы.
#[test]
fn preserves_empty_group_as_start_and_end_markers() {
    let field = to_field("Undisclosed recipients:;");
    let parsed = parse(vec![&field]).expect("empty group");
    assert_eq!(
        encode_list(&parsed).expect("IMAP group"),
        "((NIL NIL \"Undisclosed recipients\" NIL)(NIL NIL NIL NIL))"
    );
}

/// Поиск адресов видит имя группы и её mailbox components без wire-маркеров.
#[test]
fn search_projection_includes_group_phrase_and_members() {
    let field = to_field("\"A Group, East\":Ed Jones <c@a.test>;");
    let parsed = parse(vec![&field]).expect("group address-list");
    let text = search_text(&parsed).expect("search projection");
    assert!(text.contains("A Group, East"));
    assert!(text.contains("Ed Jones"));
    assert!(text.contains("c@a.test"));
}

/// Незакрытая группа и одиночная точка с запятой отклоняются явно.
#[test]
fn rejects_unbalanced_group_boundaries() {
    let open_group = to_field("Team: Alice <alice@example.test>");
    let close_group = to_field("alice@example.test;");
    assert!(parse(vec![&open_group]).is_err());
    assert!(parse(vec![&close_group]).is_err());
}

/// Удаляет вложенные comments и quoted-pair, не включая их в ENVELOPE/SEARCH.
#[test]
fn strips_nested_comments_from_name_and_addr_spec() {
    let field =
        to_field(r"John (friendly (nested \) note)) Doe <alice(comment)@(domain)example.test>");
    let parsed = parse(vec![&field]).expect("commented mailbox");
    assert_eq!(
        encode_list(&parsed).expect("ENVELOPE mailbox"),
        "((\"John Doe\" NIL \"alice\" \"example.test\"))"
    );
    let projected = search_text(&parsed).expect("address search projection");
    assert!(projected.contains("John Doe"));
    assert!(projected.contains("alice@example.test"));
    assert!(!projected.contains("friendly"));
    assert!(!projected.contains("domain"));
}

/// Отклоняет незакрытый comment вместо выдачи частичного ENVELOPE.
#[test]
fn rejects_unterminated_address_comment() {
    let field = to_field("Alice (unfinished <alice@example.test>");
    assert!(parse(vec![&field]).is_err());
}

/// Скобки внутри quoted display name остаются данными, а не comments.
#[test]
fn preserves_parentheses_inside_quoted_display_name() {
    let field = to_field("\"(Team) Lead\" <alice@example.test>");
    let parsed = parse(vec![&field]).expect("quoted display name");
    assert_eq!(
        encode_list(&parsed).expect("quoted ENVELOPE"),
        "((\"(Team) Lead\" NIL \"alice\" \"example.test\"))"
    );
}
