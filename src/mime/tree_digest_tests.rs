//! Проверки наследуемого MIME default для multipart/digest.

use super::{MediaType, parse};

#[test]
fn multipart_digest_defaults_untyped_children_to_message_rfc822() {
    let raw = b"Content-Type: multipart/digest; boundary=x\r\n\r\n--x\r\n\r\nSubject: nested\r\n\r\nmessage body\r\n--x--\r\n";
    let tree = parse(raw).expect("multipart digest");
    let child = tree.children.first().expect("digest child");

    assert_eq!(child.media_type, MediaType::MessageRfc822);
    assert_eq!(child.children.len(), 1);
    assert_eq!(
        child.children.first().map(|nested| &nested.media_type),
        Some(&MediaType::TextPlain)
    );
}

#[test]
fn untyped_multipart_mixed_child_stays_plain_text() {
    let raw =
        b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\n\r\nplain body\r\n--x--\r\n";
    let tree = parse(raw).expect("multipart mixed");
    let child = tree.children.first().expect("mixed child");

    assert_eq!(child.media_type, MediaType::TextPlain);
    assert!(child.children.is_empty());
}
