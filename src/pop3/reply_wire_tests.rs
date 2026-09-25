//! Проверки POP3 status/capability ограничений и message-body исключения.
use super::*;

#[test]
fn single_status_line_accepts_512_octets_and_rejects_513() {
    let boundary = single_line(b"+OK ", &"x".repeat(506));
    let overflow = single_line(b"+OK ", &"x".repeat(507));
    assert_eq!(boundary.len(), MAX_REPLY_LINE_SIZE);
    assert!(boundary.starts_with(b"+OK "));
    assert_eq!(overflow, LONG_REPLY);
}

#[test]
fn capability_multiline_bounds_every_wire_line() {
    let exact_line = format!("+OK {}\r\n.\r\n", "x".repeat(506)).into_bytes();
    let long_line = format!("+OK {}\r\n.\r\n", "x".repeat(507)).into_bytes();
    assert_eq!(bounded_multiline(exact_line.clone()), exact_line);
    assert_eq!(bounded_multiline(long_line), LONG_REPLY);
    assert_eq!(
        bounded_multiline(b"+OK list\r\n.\r\n".to_vec()),
        b"+OK list\r\n.\r\n"
    );
}

#[test]
fn message_body_lines_are_not_mistaken_for_status_lines() {
    let mut wire = b"+OK message follows\r\n".to_vec();
    wire.extend_from_slice(&vec![b'x'; 700]);
    wire.extend_from_slice(b"\r\n.\r\n");
    assert_eq!(message_multiline(wire.clone()), wire);

    let long_status = format!("+OK {}\r\nbody\r\n.\r\n", "x".repeat(507)).into_bytes();
    assert_eq!(message_multiline(long_status), LONG_REPLY);
}

#[test]
fn malformed_multiline_frames_fail_closed() {
    assert_eq!(
        bounded_multiline(b"+OK no terminator\r\n".to_vec()),
        LONG_REPLY
    );
    assert_eq!(
        message_multiline(b"-ERR not a message\r\n.\r\n".to_vec()),
        LONG_REPLY
    );
}

#[test]
fn action_replies_are_not_serialized_as_wire_text() {
    let action = Reply::Authenticate {
        username: "bob".into(),
        password: "secret".into(),
    };
    assert_eq!(encode_reply(action), None);
}

#[test]
fn sasl_continuation_uses_the_empty_challenge_wire_form() {
    assert_eq!(encode_reply(Reply::SaslContinue), Some(b"+ \r\n".to_vec()));
}

#[test]
fn in_use_response_code_is_serialized_as_a_bounded_rfc_2449_error() {
    let reply = Reply::ErrorCode(ResponseCode::InUse, "maildrop is locked");
    assert_eq!(
        encode_reply(reply),
        Some(b"-ERR [IN-USE] maildrop is locked\r\n".to_vec())
    );
}
