//! Проверяет точное downstream представление typed SMTP DSN параметров.

use super::{mail_suffix, recipient_suffix};
use crate::smtp::{
    MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient, RecipientDsn,
    ReturnContent,
};
use std::collections::HashSet;

/// Envelope сохраняет ENVID и RET, а origin переносит NOTIFY и исходный адрес.
#[test]
fn formats_single_origin_dsn_parameters_as_rfc3461_esmtp_values() {
    let mail = MailDsn {
        return_content: Some(ReturnContent::Headers),
        envelope_id: Some(b"trace id+1".to_vec()),
    };
    let recipient = RecipientDsn {
        notify: Some(NotifyPolicy::Conditions(HashSet::from([
            NotifyCondition::Failure,
            NotifyCondition::Success,
        ]))),
        original_recipient: Some(OriginalRecipient {
            address_type: OriginalAddressType::Rfc822,
            address: b"Alias User@example.test".to_vec(),
        }),
    };

    assert_eq!(
        mail_suffix(Some(&mail), true),
        " RET=HDRS ENVID=trace+20id+2B1"
    );
    assert_eq!(
        recipient_suffix(Some(&recipient), true),
        Some(" NOTIFY=FAILURE,SUCCESS ORCPT=rfc822;Alias+20User@example.test".to_owned())
    );
}

/// UTF-8 ORCPT משתמש ב-RFC 6533 unitext ולא ב-RFC 3461 xtext.
#[test]
fn formats_utf8_orcpt_as_ascii_safe_rfc6533_unitext() {
    let recipient = RecipientDsn {
        original_recipient: Some(OriginalRecipient {
            address_type: OriginalAddressType::Utf8,
            address: "Иван +例@example.test".as_bytes().to_vec(),
        }),
        ..RecipientDsn::default()
    };

    assert_eq!(
        recipient_suffix(Some(&recipient), true),
        Some(
            " ORCPT=utf-8;\\x{418}\\x{432}\\x{430}\\x{43D}\\x{20}\\x{2B}\\x{4F8B}@example.test"
                .to_owned()
        )
    );
}

/// Повреждённый UTF-8 и запрещённые CR/LF octets нельзя включать в команду.
#[test]
fn rejects_malformed_utf8_orcpt_before_command_serialization() {
    for address in [b"bad\xffutf8".as_slice(), b"bad\r\nRCPT"] {
        let recipient = RecipientDsn {
            original_recipient: Some(OriginalRecipient {
                address_type: OriginalAddressType::Utf8,
                address: address.to_vec(),
            }),
            ..RecipientDsn::default()
        };
        assert_eq!(recipient_suffix(Some(&recipient), true), None);
    }
}

/// Без EHLO DSN запрещённые RFC параметры не уходят downstream.
#[test]
fn omits_dsn_parameters_for_peer_without_extension() {
    let mail = MailDsn {
        return_content: Some(ReturnContent::Full),
        envelope_id: Some(b"trace".to_vec()),
    };
    let recipient = RecipientDsn {
        notify: Some(NotifyPolicy::Never),
        original_recipient: None,
    };

    assert_eq!(mail_suffix(Some(&mail), false), "");
    assert_eq!(
        recipient_suffix(Some(&recipient), false),
        Some(String::new())
    );
}
