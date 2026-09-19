//! Small, validated Sieve subset. Storage invokes it and performs delivery.
#[derive(Debug, Eq, PartialEq)]
pub enum Action {
    FileInto(String),
    Discard,
}
pub fn parse(script: &str) -> Result<Action, &'static str> {
    let s = script.trim();
    if s == "discard;" {
        return Ok(Action::Discard);
    }
    let rest = s
        .strip_prefix("fileinto ")
        .ok_or("unsupported sieve command")?
        .trim();
    let name = rest
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix("\";"))
        .ok_or("invalid fileinto")?;
    let characters_are_safe = name
        .bytes()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'));
    match (name.is_empty(), characters_are_safe) {
        (true, _) | (false, false) => return Err("unsafe mailbox"),
        (false, true) => {}
    };
    Ok(Action::FileInto(name.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_fileinto() {
        assert_eq!(
            parse("fileinto \"Invoices\";").unwrap(),
            Action::FileInto("Invoices".into())
        );
        assert!(parse("fileinto \"../x\";").is_err());
    }
}
