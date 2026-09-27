//! Формирование множества mailbox flags для SELECT и EXAMINE.

use crate::{mail_flags::KeywordFlag, store::Message};
use std::collections::BTreeSet;

/// Объединяет системные flags с keywords, уже существующими в mailbox.
pub(super) fn response(messages: &[Message]) -> String {
    let keywords = messages
        .iter()
        .flat_map(|message| message.keywords.iter())
        .map(KeywordFlag::as_str)
        .collect::<BTreeSet<_>>();
    let mut flags = vec!["\\Answered", "\\Flagged", "\\Deleted", "\\Seen", "\\Draft"];
    flags.extend(keywords);
    flags.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mail_date::MailDate, mail_flags::MessageFlags};
    use std::path::PathBuf;

    #[test]
    fn response_contains_each_existing_mailbox_keyword_once() {
        let first = message(["project", "review"]);
        let second = message(["REVIEW", "invoice"]);
        let response = super::response(&[first, second]);

        assert_eq!(
            response,
            "\\Answered \\Flagged \\Deleted \\Seen \\Draft INVOICE PROJECT REVIEW"
        );
    }

    fn message<const COUNT: usize>(values: [&str; COUNT]) -> Message {
        Message {
            uid: 1,
            path: PathBuf::new(),
            bytes: Vec::new(),
            flags: MessageFlags::default(),
            keywords: values.into_iter().filter_map(KeywordFlag::parse).collect(),
            internal_date: MailDate::from_unix_seconds(0),
        }
    }
}
