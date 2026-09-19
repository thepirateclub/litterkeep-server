//! Tagged IMAP command decoding. Text is converted to an enum at the boundary.
#[derive(Debug, Eq, PartialEq)]
pub struct TaggedCommand {
    pub tag: String,
    pub command: Command,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Capability,
    Login,
    Logout,
    Select,
    Examine,
    Fetch(usize),
    Unsupported,
}
enum Verb {
    Capability,
    Login,
    Logout,
    Select,
    Examine,
    Fetch,
    Unsupported,
}
pub fn parse(line: &str) -> TaggedCommand {
    let mut words = line.split_whitespace();
    let tag = words.next().unwrap_or("*").into();
    let verb = parse_verb(words.next().unwrap_or(""));
    let command = match verb {
        Verb::Capability => Command::Capability,
        Verb::Login => Command::Login,
        Verb::Logout => Command::Logout,
        Verb::Select => Command::Select,
        Verb::Examine => Command::Examine,
        Verb::Fetch => match words.next().and_then(|value| value.parse().ok()) {
            Some(number) => Command::Fetch(number),
            None => Command::Unsupported,
        },
        Verb::Unsupported => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}
fn parse_verb(text: &str) -> Verb {
    match text.to_ascii_uppercase().as_str() {
        "CAPABILITY" => Verb::Capability,
        "LOGIN" => Verb::Login,
        "LOGOUT" => Verb::Logout,
        "SELECT" => Verb::Select,
        "EXAMINE" => Verb::Examine,
        "FETCH" => Verb::Fetch,
        _ => Verb::Unsupported,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_fetch_as_enum() {
        assert_eq!(
            parse("a FETCH 3 (RFC822)\r\n"),
            TaggedCommand {
                tag: "a".into(),
                command: Command::Fetch(3)
            }
        );
    }
}
