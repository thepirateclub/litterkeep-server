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
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("unsafe mailbox");
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
