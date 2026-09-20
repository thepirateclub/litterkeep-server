//! Текстовые, связанные и исторические письма полного seed набора.

use super::{Delivery, Fixture, Mailbox, Owner, fixture, message};

pub(super) fn fixtures() -> Vec<Fixture> {
    let mut result = vec![
        plain(
            "seed-001-plain-en",
            "Seed plain English",
            b"Hello from the repeatable seed.\r\n",
        ),
        plain(
            "seed-002-russian",
            "Русское письмо",
            "Привет, Bob! Это проверка UTF-8.\r\n".as_bytes(),
        ),
        alternative(),
        historical(
            "seed-004-sent",
            Mailbox::Sent,
            "\\Seen",
            "Historical sent item",
        ),
        historical(
            "seed-005-draft",
            Mailbox::Drafts,
            "\\Draft \\Flagged",
            "Flagged draft",
        ),
        plain(
            "seed-006-emoji",
            "Emoji mailbox",
            "Status: ✅ 📬 🚀\r\n".as_bytes(),
        ),
        related(
            "seed-007-reply",
            "Re: Seed plain English",
            "In-Reply-To: <seed-001-plain-en@litterkeep.test>\r\nReferences: <seed-001-plain-en@litterkeep.test>\r\n",
        ),
        forwarded(),
        related(
            "seed-009-bcc",
            "Envelope Bcc fixture",
            "Bcc: hidden@litterkeep.test\r\n",
        ),
        historical(
            "seed-010-archive",
            Mailbox::Archive,
            "\\Seen",
            "Archived history",
        ),
    ];
    for number in 11_u8..=28 {
        let id = format!("seed-{number:03}-thread");
        let subject = format!("Thread sample {number:02}");
        let body = format!("Deterministic bounded fixture number {number}.\r\n");
        let headers = match number % 3 {
            0 => "In-Reply-To: <seed-001-plain-en@litterkeep.test>\r\n",
            1 => "X-Seed-Category: work\r\n",
            _ => "X-Seed-Category: personal\r\n",
        };
        result.push(fixture(
            &id,
            Mailbox::Inbox,
            Owner::Bob,
            Delivery::Smtp,
            "",
            message(
                &id,
                &subject,
                headers,
                "text/plain; charset=utf-8",
                body.as_bytes(),
            ),
        ));
    }
    result
}

fn plain(id: &str, subject: &str, body: &[u8]) -> Fixture {
    fixture(
        id,
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(id, subject, "", "text/plain; charset=utf-8", body),
    )
}

fn related(id: &str, subject: &str, headers: &str) -> Fixture {
    fixture(
        id,
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(
            id,
            subject,
            headers,
            "text/plain; charset=utf-8",
            b"Related message.\r\n",
        ),
    )
}

fn historical(id: &str, mailbox: Mailbox, flags: &str, subject: &str) -> Fixture {
    fixture(
        id,
        mailbox,
        Owner::Alice,
        Delivery::ImapAppend,
        flags,
        message(
            id,
            subject,
            "",
            "text/plain; charset=utf-8",
            b"Stored through IMAP APPEND.\r\n",
        ),
    )
}

fn alternative() -> Fixture {
    let body = b"--seed-alt\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlain version.\r\n--seed-alt\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>HTML <strong>version</strong>.</p>\r\n--seed-alt--\r\n";
    fixture(
        "seed-003-alternative",
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(
            "seed-003-alternative",
            "Plain and HTML",
            "",
            "multipart/alternative; boundary=seed-alt",
            body,
        ),
    )
}

fn forwarded() -> Fixture {
    let body = b"--seed-forward\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nForwarded below.\r\n--seed-forward\r\nContent-Type: message/rfc822\r\n\r\nFrom: Bob <bob@litterkeep.test>\r\nTo: Alice <alice@litterkeep.test>\r\nSubject: Original message\r\n\r\nOriginal body.\r\n--seed-forward--\r\n";
    fixture(
        "seed-008-forward",
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(
            "seed-008-forward",
            "Fwd: Original message",
            "",
            "multipart/mixed; boundary=seed-forward",
            body,
        ),
    )
}
