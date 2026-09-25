use super::{Command, folder::ListPattern, parse};

/// Bare LIST pattern принимает wildcard, но отвергает atom-special brace.
#[test]
fn list_pattern_accepts_wildcard_and_rejects_unquoted_specials() {
    assert_eq!(
        parse("A1 LIST \"\" *\r\n").command,
        Command::List {
            pattern: ListPattern::All,
            returns: super::folder::ListReturnOptions::default(),
        }
    );
    assert_eq!(
        parse("A2 LIST \"\" folder{invalid\r\n").command,
        Command::Unsupported
    );
}

/// Pattern с atom-special допустим в quoted form, reference использует astring.
#[test]
fn list_pattern_and_reference_keep_distinct_string_grammars() {
    assert_eq!(
        parse("A3 LIST \"\" \"folder{valid\"\r\n").command,
        Command::List {
            pattern: ListPattern::Exact("folder{valid".into()),
            returns: super::folder::ListReturnOptions::default(),
        }
    );
    assert_eq!(
        parse("A4 LIST reference% \"\"\r\n").command,
        Command::Unsupported
    );
}
