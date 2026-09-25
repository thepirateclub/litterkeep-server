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

/// RFC 5322 допускает CFWS после завершающего разделителя группы.
#[test]
fn accepts_comment_after_group_terminator() {
    let field = to_field("Team:alice@example.test; (group closed)");
    let parsed = parse(vec![&field]).expect("group with trailing comment");
    assert_eq!(
        encode_list(&parsed).expect("group ENVELOPE"),
        "((NIL NIL \"Team\" NIL)(NIL NIL \"alice\" \"example.test\")(NIL NIL NIL NIL))"
    );
}

/// Реальный адрес после точки с запятой не маскируется под завершающий CFWS.
#[test]
fn rejects_content_after_group_terminator() {
    let field = to_field("Team:alice@example.test; bob@example.test");
    assert!(parse(vec![&field]).is_err());
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

/// Снимает quoted-pair из имени до сериализации ENVELOPE.
#[test]
fn decodes_quoted_pairs_in_display_name() {
    let field = to_field(r#""Team \"North\" and \\ Group" <alice@example.test>"#);
    let parsed = parse(vec![&field]).expect("quoted display name");
    assert_eq!(
        encode_list(&parsed).expect("quoted-pair ENVELOPE"),
        r#"(("Team \"North\" and \\ Group" NIL "alice" "example.test"))"#
    );
}

/// Декодирует RFC 2047 слова только в unquoted phrase позиции имени.
#[test]
fn decodes_encoded_words_in_display_name_phrase() {
    let field = to_field("=?UTF-8?Q?Alice?= =?UTF-8?Q?_North?= <alice@example.test>");
    let parsed = parse(vec![&field]).expect("encoded display name");
    assert_eq!(
        encode_list(&parsed).expect("decoded ENVELOPE"),
        "((\"Alice North\" NIL \"alice\" \"example.test\"))"
    );
}

/// Некорректный encoded-word остаётся видимым текстом, не ломая письмо.
#[test]
fn preserves_malformed_encoded_word_in_display_name() {
    let field = to_field("=?UTF-8?Q?bad=ZZ?= <alice@example.test>");
    let parsed = parse(vec![&field]).expect("malformed encoded-word is ordinary text");
    assert_eq!(
        encode_list(&parsed).expect("visible malformed word"),
        "((\"=?UTF-8?Q?bad=ZZ?=\" NIL \"alice\" \"example.test\"))"
    );
}

/// Encoded-word внутри quoted-string не интерпретируется как RFC 2047.
#[test]
fn preserves_encoded_word_inside_quoted_display_name() {
    let field = to_field("\"=?UTF-8?Q?Alice?=\" <alice@example.test>");
    let parsed = parse(vec![&field]).expect("quoted display name");
    assert_eq!(
        encode_list(&parsed).expect("quoted word remains literal"),
        "((\"=?UTF-8?Q?Alice?=\" NIL \"alice\" \"example.test\"))"
    );
}

/// Разделяет addr-spec по внешнему @, не по символу внутри quoted local-part.
#[test]
fn preserves_at_sign_inside_quoted_local_part() {
    let field = to_field(r#"Quoted <"a@b"@example.test>"#);
    let parsed = parse(vec![&field]).expect("quoted local-part");
    assert_eq!(
        encode_list(&parsed).expect("ENVELOPE quoted local-part"),
        "((\"Quoted\" NIL \"\\\"a@b\\\"\" \"example.test\"))"
    );
    let projected = search_text(&parsed).expect("SEARCH quoted local-part");
    assert!(projected.contains("\"a@b\"@example.test"));
}

/// Отклоняет несколько внешних разделителей вместо неверной сегментации.
#[test]
fn rejects_multiple_addr_spec_separators() {
    let field = to_field("Alice <alice@example.test@other.test>");
    assert!(parse(vec![&field]).is_err());
}

/// RFC 5322 obsolete source-route игнорируется, целевой mailbox сохраняется.
#[test]
fn ignores_obsolete_source_route_in_envelope_projection() {
    let field = to_field("Legacy <@old.example,@older.example:alice@example.test>");
    let parsed = parse(vec![&field]).expect("obsolete source-routed mailbox");
    assert_eq!(
        encode_list(&parsed).expect("routed ENVELOPE"),
        "((\"Legacy\" NIL \"alice\" \"example.test\"))"
    );
    let projection = search_text(&parsed).expect("routed address search projection");
    assert!(projection.contains("alice@example.test"));
    assert!(!projection.contains("old.example"));
}

/// Сохраняет допустимую пустую необязательную route-position после запятой.
#[test]
fn accepts_optional_empty_tail_in_obsolete_source_route() {
    let field = to_field("Legacy <@old.example,:alice@example.test>");
    let parsed = parse(vec![&field]).expect("obsolete source-route trailing comma");
    assert_eq!(
        encode_list(&parsed).expect("routed ENVELOPE"),
        "((\"Legacy\" NIL \"alice\" \"example.test\"))"
    );
}

/// Не принимает пустой маршрут или отсутствие адреса после route delimiter.
#[test]
fn rejects_malformed_obsolete_source_route() {
    let empty_domain = to_field("Legacy <@,:alice@example.test>");
    let empty_target = to_field("Legacy <@old.example:>");
    assert!(parse(vec![&empty_domain]).is_err());
    assert!(parse(vec![&empty_target]).is_err());
}

/// Удаляет obsolete CFWS вокруг dot separators local-part и domain.
#[test]
fn normalizes_obsolete_cfws_around_addr_spec_dots() {
    let field = to_field("Legacy <first (local) . (separator) last @ example . test>");
    let parsed = parse(vec![&field]).expect("obsolete dotted address");
    assert_eq!(
        encode_list(&parsed).expect("obsolete address ENVELOPE"),
        "((\"Legacy\" NIL \"first.last\" \"example.test\"))"
    );
    let projection = search_text(&parsed).expect("obsolete address SEARCH");
    assert!(projection.contains("first.last@example.test"));
}

/// Пробелы и точки внутри quoted local-part не схлопываются как CFWS.
#[test]
fn preserves_quoted_local_part_content_during_dot_normalization() {
    let field = to_field(r#"Quoted <"a . b"@example.test>"#);
    let parsed = parse(vec![&field]).expect("quoted local-part");
    assert_eq!(
        encode_list(&parsed).expect("quoted local-part ENVELOPE"),
        "((\"Quoted\" NIL \"\\\"a . b\\\"\" \"example.test\"))"
    );
}

/// Не нормализует точки внутри bracketed domain-literal.
#[test]
fn preserves_domain_literal_during_dot_normalization() {
    let field = to_field("Literal <host@[127.0.0.1]>");
    let parsed = parse(vec![&field]).expect("domain-literal address");
    assert_eq!(
        encode_list(&parsed).expect("domain-literal ENVELOPE"),
        "((\"Literal\" NIL \"host\" \"[127.0.0.1]\"))"
    );
}

/// RFC 5322 obsolete address-list null members не создают ложных адресов.
#[test]
fn ignores_obsolete_null_members_in_address_list() {
    let field = to_field(",,alice@example.test,,bob@example.test,");
    let parsed = parse(vec![&field]).expect("obsolete null address members");
    assert_eq!(
        encode_list(&parsed).expect("null-member ENVELOPE"),
        "((NIL NIL \"alice\" \"example.test\")(NIL NIL \"bob\" \"example.test\"))"
    );
}
