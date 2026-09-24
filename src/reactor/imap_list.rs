//! Формирование IMAP LIST и bounded wildcard matcher.

use super::*;

pub(super) fn listed(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    special_use_only: bool,
    result: io::Result<Vec<crate::store::FolderInfo>>,
) -> io::Result<()> {
    let folders = match result {
        Ok(folders) => folders,
        Err(_) => return session.push(format!("{tag} NO LIST failed\r\n").as_bytes()),
    };
    for folder in folders
        .iter()
        .filter(|folder| listed_folder(folder, &pattern, special_use_only))
    {
        let attributes = imap_folders::special_use(folder.special_use);
        let name = imap_folders::quote(&folder.name);
        session.push(format!("* LIST ({attributes}) \"/\" {name}\r\n").as_bytes())?;
    }
    session.push(format!("{tag} OK LIST completed\r\n").as_bytes())
}

pub(super) fn matches_pattern(name: &str, pattern: &imap::folder::ListPattern) -> bool {
    match pattern {
        imap::folder::ListPattern::Root | imap::folder::ListPattern::RootReference(_) => false,
        imap::folder::ListPattern::All => true,
        imap::folder::ListPattern::OneLevel => !name.contains('/'),
        imap::folder::ListPattern::Exact(expected) => name == expected,
        imap::folder::ListPattern::Wildcard(pattern) => wildcard_matches(pattern, name),
    }
}

fn listed_folder(
    folder: &crate::store::FolderInfo,
    pattern: &imap::folder::ListPattern,
    special_use_only: bool,
) -> bool {
    match special_use_only {
        true => match folder.special_use.is_some() {
            true => matches_pattern(&folder.name, pattern),
            false => false,
        },
        false => matches_pattern(&folder.name, pattern),
    }
}

/// NFA обход wildcard ограничен длиной имени и шаблона.
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    let mut states = closure(vec![pattern]);
    for character in name.chars() {
        let mut next = Vec::new();
        for state in states {
            let Some((token, rest)) = split_first(state) else {
                continue;
            };
            match token {
                '*' => add_state(&mut next, state),
                '%' if character != '/' => add_state(&mut next, state),
                expected if expected == character => add_state(&mut next, rest),
                _ => {}
            }
        }
        states = closure(next);
    }
    closure(states).iter().any(|state| state.is_empty())
}

fn closure(mut states: Vec<&str>) -> Vec<&str> {
    let mut cursor = 0;
    while let Some(state) = states.get(cursor).copied() {
        if let Some(('*' | '%', rest)) = split_first(state) {
            add_state(&mut states, rest);
        }
        cursor = cursor.saturating_add(1);
    }
    states
}

fn split_first(value: &str) -> Option<(char, &str)> {
    let character = value.chars().next()?;
    Some((character, value.strip_prefix(character)?))
}

fn add_state<'a>(states: &mut Vec<&'a str>, state: &'a str) {
    if !states.contains(&state) {
        states.push(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_stops_at_delimiter_and_star_crosses_it() {
        let percent = imap::folder::ListPattern::Wildcard("Invoices/%".into());
        let star = imap::folder::ListPattern::Wildcard("Invoices/*".into());
        assert!(matches_pattern("Invoices/2026", &percent));
        assert!(!matches_pattern("Invoices/2026/January", &percent));
        assert!(matches_pattern("Invoices/2026/January", &star));
    }
}
