//! Codec и recovery tests transaction-linked SMTP DSN sidecar.

use super::*;
use crate::smtp::{
    MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient, RcptTo,
    RecipientDsn, ReturnContent,
};
use std::collections::HashSet;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn envelope() -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "alice@test".into(),
        mail: MailDsn {
            return_content: Some(ReturnContent::Full),
            envelope_id: Some(b"job 42".to_vec()),
        },
        recipients: vec![RcptTo {
            address: "bob@test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Success, NotifyCondition::Failure]
                        .into_iter()
                        .collect(),
                )),
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Rfc822,
                    address: b"bob@test".to_vec(),
                }),
            },
        }],
    }
}

fn journal_root() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("litterkeep-smtp-dsn-{unique}"));
    fs::create_dir_all(root.join("journal")).expect("create journal directory");
    root
}

/// Round trip сохраняет sender, MAIL policy и независимые recipient поля.
#[test]
fn frame_round_trip_preserves_typed_envelope() {
    let expected = envelope();
    let bytes = encode::frame(7, &expected).expect("encode envelope");
    let decoded = decode::scan(&bytes).expect("decode frame");
    assert_eq!(decoded, vec![(7, expected)]);
}

/// Loader скрывает целый envelope frame без общего transaction commit.
#[test]
fn load_ignores_complete_uncommitted_sidecar_records() {
    let root = journal_root();
    append(&root, 7, &envelope()).expect("append sidecar frame");
    assert!(
        load(&root, &HashSet::new())
            .expect("load uncommitted")
            .is_empty()
    );
    let committed = [7].into_iter().collect();
    assert_eq!(
        load(&root, &committed).expect("load committed").get(&7),
        Some(&envelope())
    );
    let _ = fs::remove_dir_all(root);
}

/// Recovery обрезает неполный tail, но не скрывает повреждённый полный frame.
#[test]
fn recovery_trims_incomplete_tail_and_rejects_checksum_corruption() {
    let root = journal_root();
    append(&root, 7, &envelope()).expect("append valid frame");
    let path = root.join("journal/smtp-dsn.log");
    let original_length = fs::metadata(&path).expect("metadata").len();
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open sidecar");
    file.write_all(b"LKSD\0").expect("append partial frame");
    recover(&root).expect("trim incomplete frame");
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        original_length
    );

    let mut corrupt = fs::read(&path).expect("read sidecar");
    let last = corrupt.last_mut().expect("complete frame");
    *last ^= 1;
    assert!(decode::scan(&corrupt).is_err());
    let _ = fs::remove_dir_all(root);
}
