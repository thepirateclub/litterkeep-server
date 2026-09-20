//! Детерминированные MIME fixtures первого сетевого seed checkpoint-а.

/// Протокол, которым fixture должна попасть в целевую папку.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Delivery {
    Smtp,
    ImapAppend,
}

/// Типизированная стандартная папка вместо строкового состояния seed-а.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mailbox {
    Inbox,
    Sent,
    Drafts,
}

/// Локальный account, которому принадлежит проверяемая папка.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Owner {
    Alice,
    Bob,
}

impl Mailbox {
    pub(super) fn wire(self) -> &'static str {
        match self {
            Self::Inbox => "INBOX",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
        }
    }
}

/// Полностью воспроизводимое письмо с устойчивым seed identifier.
pub(super) struct Fixture {
    pub id: &'static str,
    pub recipient: &'static str,
    pub mailbox: Mailbox,
    pub owner: Owner,
    pub delivery: Delivery,
    pub flags: &'static str,
    pub message: &'static [u8],
}

/// Первые пять fixtures покрывают SMTP, APPEND, Unicode, alternative и attachment.
pub(super) fn initial() -> [Fixture; 5] {
    [
        Fixture {
            id: "seed-001-plain-en",
            recipient: "bob@litterkeep.test",
            mailbox: Mailbox::Inbox,
            owner: Owner::Bob,
            delivery: Delivery::Smtp,
            flags: "",
            message: b"From: Alice <alice@litterkeep.test>\r\nTo: Bob <bob@litterkeep.test>\r\nSubject: Seed plain English\r\nX-Litterkeep-Fixture: seed-001-plain-en\r\nMessage-ID: <seed-001@litterkeep.test>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello from the repeatable seed.\r\n",
        },
        Fixture {
            id: "seed-002-russian",
            recipient: "bob@litterkeep.test",
            mailbox: Mailbox::Inbox,
            owner: Owner::Bob,
            delivery: Delivery::Smtp,
            flags: "",
            message: "From: Alice <alice@litterkeep.test>\r\nTo: Bob <bob@litterkeep.test>\r\nSubject: Русское письмо ✉\r\nX-Litterkeep-Fixture: seed-002-russian\r\nMessage-ID: <seed-002@litterkeep.test>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\nПривет, Bob! Это проверка UTF-8.\r\n".as_bytes(),
        },
        Fixture {
            id: "seed-003-alternative",
            recipient: "bob@litterkeep.test",
            mailbox: Mailbox::Inbox,
            owner: Owner::Bob,
            delivery: Delivery::Smtp,
            flags: "",
            message: b"From: Alice <alice@litterkeep.test>\r\nTo: Bob <bob@litterkeep.test>\r\nSubject: Plain and HTML\r\nX-Litterkeep-Fixture: seed-003-alternative\r\nMessage-ID: <seed-003@litterkeep.test>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=seed-alt\r\n\r\n--seed-alt\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlain version.\r\n--seed-alt\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>HTML <strong>version</strong>.</p>\r\n--seed-alt--\r\n",
        },
        Fixture {
            id: "seed-004-sent",
            recipient: "alice@litterkeep.test",
            mailbox: Mailbox::Sent,
            owner: Owner::Alice,
            delivery: Delivery::ImapAppend,
            flags: "\\Seen",
            message: b"From: Alice <alice@litterkeep.test>\r\nTo: Bob <bob@litterkeep.test>\r\nSubject: Historical sent item\r\nX-Litterkeep-Fixture: seed-004-sent\r\nMessage-ID: <seed-004@litterkeep.test>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nStored through IMAP APPEND.\r\n",
        },
        Fixture {
            id: "seed-005-draft-attachment",
            recipient: "alice@litterkeep.test",
            mailbox: Mailbox::Drafts,
            owner: Owner::Alice,
            delivery: Delivery::ImapAppend,
            flags: "\\Draft",
            message: b"From: Alice <alice@litterkeep.test>\r\nTo: QA <qa@litterkeep.test>\r\nSubject: Draft with attachment\r\nX-Litterkeep-Fixture: seed-005-draft-attachment\r\nMessage-ID: <seed-005@litterkeep.test>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=seed-mixed\r\n\r\n--seed-mixed\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nDraft body.\r\n--seed-mixed\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=seed.bin\r\nContent-Transfer-Encoding: base64\r\n\r\nAAECA/8=\r\n--seed-mixed--\r\n",
        },
    ]
}
