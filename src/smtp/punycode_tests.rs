//! Проверки RFC-векторов и защитных границ Punycode codec.

use super::{Error, decode, encode};

/// Все примеры RFC 3492 §7.1 проверяют Bootstring независимо от IDNA-профиля.
#[test]
fn matches_every_rfc_3492_section_7_1_sample() {
    for &(unicode, punycode) in RFC_3492_SAMPLE_STRINGS {
        assert_eq!(encode(unicode), Ok(punycode.to_owned()));
        assert_eq!(decode(punycode), Ok(unicode.to_owned()));
    }
}

/// Официальные пары RFC: I учитывает исправление RFC Editor Erratum 3026.
/// Пример S намеренно проверяет Bootstring на произвольном ASCII, не IDNA.
const RFC_3492_SAMPLE_STRINGS: &[(&str, &str)] = &[
    (
        "\u{0644}\u{064A}\u{0647}\u{0645}\u{0627}\u{0628}\u{062A}\u{0643}\u{0644}\u{0645}\u{0648}\u{0634}\u{0639}\u{0631}\u{0628}\u{064A}\u{061F}",
        "egbpdaj6bu4bxfgehfvwxn",
    ),
    ("他们为什么不说中文", "ihqwcrb4cv8a8dqg056pqjye"),
    ("他們爲什麽不說中文", "ihqwctvzc91f659drss3x8bo0yb"),
    (
        "Pro\u{010D}prost\u{011B}nemluv\u{00ED}\u{010D}esky",
        "Proprostnemluvesky-uyb24dma41a",
    ),
    (
        "\u{05DC}\u{05DE}\u{05D4}\u{05D4}\u{05DD}\u{05E4}\u{05E9}\u{05D5}\u{05D8}\u{05DC}\u{05D0}\u{05DE}\u{05D3}\u{05D1}\u{05E8}\u{05D9}\u{05DD}\u{05E2}\u{05D1}\u{05E8}\u{05D9}\u{05EA}",
        "4dbcagdahymbxekheh6e0a7fei0b",
    ),
    (
        "\u{092F}\u{0939}\u{0932}\u{094B}\u{0917}\u{0939}\u{093F}\u{0928}\u{094D}\u{0926}\u{0940}\u{0915}\u{094D}\u{092F}\u{094B}\u{0902}\u{0928}\u{0939}\u{0940}\u{0902}\u{092C}\u{094B}\u{0932}\u{0938}\u{0915}\u{0924}\u{0947}\u{0939}\u{0948}\u{0902}",
        "i1baa7eci9glrd9b2ae1bj0hfcgg6iyaf8o0a1dig0cd",
    ),
    (
        "\u{306A}\u{305C}\u{307F}\u{3093}\u{306A}\u{65E5}\u{672C}\u{8A9E}\u{3092}\u{8A71}\u{3057}\u{3066}\u{304F}\u{308C}\u{306A}\u{3044}\u{306E}\u{304B}",
        "n8jok5ay5dzabd5bym9f0cm5685rrjetr6pdxa",
    ),
    (
        "\u{C138}\u{ACC4}\u{C758}\u{BAA8}\u{B4E0}\u{C0AC}\u{B78C}\u{B4E4}\u{C774}\u{D55C}\u{AD6D}\u{C5B4}\u{B97C}\u{C774}\u{D574}\u{D55C}\u{B2E4}\u{BA74}\u{C5BC}\u{B9C8}\u{B098}\u{C88B}\u{C744}\u{AE4C}",
        "989aomsvi5e83db1d2a355cv1e0vak1dwrv93d5xbh15a0dt30a5jpsd879ccm6fea98c",
    ),
    (
        "\u{043F}\u{043E}\u{0447}\u{0435}\u{043C}\u{0443}\u{0436}\u{0435}\u{043E}\u{043D}\u{0438}\u{043D}\u{0435}\u{0433}\u{043E}\u{0432}\u{043E}\u{0440}\u{044F}\u{0442}\u{043F}\u{043E}\u{0440}\u{0443}\u{0441}\u{0441}\u{043A}\u{0438}",
        "b1abfaaepdrnnbgefbadotcwatmq2g4l",
    ),
    (
        "Porqu\u{00E9}nopuedensimplementehablarenEspa\u{00F1}ol",
        "PorqunopuedensimplementehablarenEspaol-fmd56a",
    ),
    (
        "\u{0054}\u{1EA1}\u{0069}\u{0073}\u{0061}\u{006F}\u{0068}\u{1ECD}\u{006B}\u{0068}\u{00F4}\u{006E}\u{0067}\u{0074}\u{0068}\u{1EC3}\u{0063}\u{0068}\u{1EC9}\u{006E}\u{00F3}\u{0069}\u{0074}\u{0069}\u{1EBF}\u{006E}\u{0067}\u{0056}\u{0069}\u{1EC7}\u{0074}",
        "TisaohkhngthchnitingVit-kjcr8268qyxafd2f1b9g",
    ),
    ("3年B組金八先生", "3B-ww4c5e180e575a65lsy2b"),
    (
        "\u{5B89}\u{5BA4}\u{5948}\u{7F8E}\u{6075}-with-SUPER-MONKEYS",
        "-with-SUPER-MONKEYS-pc58ag80a8qai00g7n9n",
    ),
    (
        "Hello-Another-Way-それぞれの場所",
        "Hello-Another-Way--fc4qua05auwb3674vfr0b",
    ),
    ("ひとつ屋根の下2", "2-u9tlzr9756bt3uc0v"),
    ("MajiでKoiする5秒前", "MajiKoi5-783gue6qz075azm5e"),
    ("パフィーdeルンバ", "de-jg4avhby1noc0d"),
    ("そのスピードで", "d9juau41awczczp"),
    ("-> $1.00 <-", "-> $1.00 <--"),
];

/// ASCII-коды сохраняются, а смешанная строка проходит обратное преобразование.
#[test]
fn preserves_basic_code_points_and_round_trips() {
    for value in ["bücher", "mañana", "A-例え-9"] {
        let encoded = encode(value).expect("valid Unicode scalar input");
        assert_eq!(decode(&encoded), Ok(value.to_owned()));
    }
    assert_eq!(encode(""), Ok(String::new()));
    assert_eq!(encode("ascii"), Ok("ascii-".to_owned()));
    assert_eq!(encode("bücher"), Ok("bcher-kva".to_owned()));
}

/// Декодер отвергает неверные цифры, усечённые числа и surrogate code points.
#[test]
fn rejects_malformed_or_overflowing_sequences() {
    for value in ["a*", "z-zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"] {
        assert!(matches!(
            decode(value),
            Err(Error::InvalidInput | Error::Overflow)
        ));
    }
    assert_eq!(decode("abc-"), Ok("abc".to_owned()));
    assert_eq!(decode("é"), Err(Error::InvalidInput));
    assert_eq!(encode(&"a".repeat(1025)), Err(Error::InputTooLong));
}
