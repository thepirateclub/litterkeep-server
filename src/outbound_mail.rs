//! Построение ESMTP MAIL FROM с согласованными параметрами расширений.

use super::{
    EhloCapabilities, EhloExtension, SizeAdvertisement,
    data::{has_8bit_body, transmitted_message_size},
};

#[path = "outbound_header_profile.rs"]
pub(crate) mod header_profile;
use self::header_profile::has_utf8_headers;

/// Исход выбора outbound MAIL: отправить команду либо завершить job permanent.
pub(crate) enum MailCommand {
    Ready(String),
    PermanentFailure,
}

/// Создаёт MAIL FROM и объявляет только параметры, согласованные через EHLO.
pub(crate) fn build_mail_command(
    sender: &str,
    envelope_requires_smtp_utf8: bool,
    message: &[u8],
    capabilities: &EhloCapabilities,
    dsn: Option<&crate::smtp::MailDsn>,
) -> MailCommand {
    let contains_binary_transfer = crate::mime::tree::parse(message)
        .is_ok_and(|tree| tree.contains_binary_transfer_encoding());
    if contains_binary_transfer
        && !(capabilities.supports(EhloExtension::BinaryMime)
            && capabilities.supports(EhloExtension::Chunking))
    {
        return MailCommand::PermanentFailure;
    }
    let declared_octets = match contains_binary_transfer {
        true => message.len(),
        false => transmitted_message_size(message),
    };
    let declared_size = u64::try_from(declared_octets).unwrap_or(u64::MAX);
    let size_parameter = match capabilities.size_advertisement() {
        SizeAdvertisement::NotAdvertised => String::new(),
        SizeAdvertisement::NoFixedLimit => format!(" SIZE={declared_size}"),
        SizeAdvertisement::Maximum(maximum) if declared_size > maximum => {
            return MailCommand::PermanentFailure;
        }
        SizeAdvertisement::Maximum(_) | SizeAdvertisement::MaximumAboveU64 => {
            format!(" SIZE={declared_size}")
        }
    };
    let utf8_headers = has_utf8_headers(message);
    let body_requires_8bit = !contains_binary_transfer && (has_8bit_body(message) || utf8_headers);
    let body_parameter = match (contains_binary_transfer, body_requires_8bit) {
        (true, _) => " BODY=BINARYMIME",
        (false, true) if capabilities.supports(EhloExtension::EightBitMime) => " BODY=8BITMIME",
        (false, true) => return MailCommand::PermanentFailure,
        (false, false) => "",
    };
    let smtp_utf8_required = utf8_headers || envelope_requires_smtp_utf8;
    let utf8_header_parameter = match smtp_utf8_required {
        true if capabilities.supports(EhloExtension::SmtpUtf8)
            && capabilities.supports(EhloExtension::EightBitMime) =>
        {
            " SMTPUTF8"
        }
        true => return MailCommand::PermanentFailure,
        false => "",
    };
    let command = format!(
        "MAIL FROM:<{sender}>{size_parameter}{body_parameter}{utf8_header_parameter}{}\r\n",
        super::dsn_mail_suffix(dsn, capabilities.supports(EhloExtension::Dsn))
    );
    let command_limit = crate::smtp::command_line_limit(command.as_bytes());
    match command.len() <= command_limit {
        true => MailCommand::Ready(command),
        false => MailCommand::PermanentFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::{MailCommand, build_mail_command};
    use crate::outbound::{SizeAdvertisement, read_ehlo};
    use std::io::Cursor;

    fn capabilities(reply: &[u8]) -> crate::outbound::EhloCapabilities {
        read_ehlo(&mut Cursor::new(reply)).expect("EHLO capabilities")
    }

    #[test]
    fn size_is_omitted_when_peer_did_not_advertise_the_extension() {
        let peer = capabilities(b"250-peer.test\r\n250 PIPELINING\r\n");
        assert!(matches!(
            build_mail_command("alice@test", false, b"Subject: x\r\n\r\nbody", &peer, None),
            MailCommand::Ready(command) if command == "MAIL FROM:<alice@test>\r\n"
        ));
    }

    #[test]
    fn declared_size_includes_the_added_terminal_crlf_but_not_dot_stuffing() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 1024\r\n");
        let message = b"Subject: x\r\n\r\n.body";
        let expected_size = message.len().saturating_add(2);
        assert!(matches!(
            build_mail_command("alice@test", false, message, &peer, None),
            MailCommand::Ready(command)
                if command == format!("MAIL FROM:<alice@test> SIZE={expected_size}\r\n")
        ));
    }

    #[test]
    fn fixed_size_limit_rejects_before_creating_mail_command() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 10\r\n");
        assert!(matches!(
            build_mail_command("alice@test", false, b"Subject: x\r\n\r\nbody", &peer, None),
            MailCommand::PermanentFailure
        ));
    }

    #[test]
    fn size_and_8bitmime_parameters_are_combined_in_one_mail_command() {
        let peer = capabilities(b"250-peer.test\r\n250-SIZE 1024\r\n250 8BITMIME\r\n");
        let message = "Content-Transfer-Encoding: 8bit\r\n\r\nПривет".as_bytes();
        let size = message.len().saturating_add(2);
        assert!(matches!(
            build_mail_command("alice@test", false, message, &peer, None),
            MailCommand::Ready(command)
                if command == format!(
                    "MAIL FROM:<alice@test> SIZE={size} BODY=8BITMIME\r\n"
                )
        ));
    }

    #[test]
    fn advertised_size_zero_means_unlimited_and_still_declares_octets() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 0\r\n");
        assert_eq!(peer.size_advertisement(), SizeAdvertisement::NoFixedLimit);
        assert!(matches!(
            build_mail_command("alice@test", false, b"body", &peer, None),
            MailCommand::Ready(command) if command == "MAIL FROM:<alice@test> SIZE=6\r\n"
        ));
    }

    #[test]
    fn size_above_u64_is_retained_and_declared_for_bounded_local_messages() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 18446744073709551616\r\n");
        assert!(matches!(
            build_mail_command("alice@test", false, b"body", &peer, None),
            MailCommand::Ready(command)
                if command == "MAIL FROM:<alice@test> SIZE=6\r\n"
        ));
    }

    /// MIME `binary` не должен случайно уйти как 7BIT/8BIT через DATA.
    #[test]
    fn binary_transfer_encoding_requires_both_binarymime_and_chunking() {
        let peer = capabilities(b"250-peer.test\r\n250-8BITMIME\r\n250 CHUNKING\r\n");
        let message = b"MIME-Version: 1.0\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xffpayload";

        assert!(matches!(
            build_mail_command("alice@test", false, message, &peer, None),
            MailCommand::PermanentFailure
        ));
    }

    /// BINARYMIME MAIL использует точную длину BDAT payload без DATA CRLF.
    #[test]
    fn binary_transfer_encoding_declares_binarymime_and_exact_size() {
        let peer =
            capabilities(b"250-peer.test\r\n250-BINARYMIME\r\n250-CHUNKING\r\n250 SIZE 1024\r\n");
        let message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xff";

        assert!(matches!(
            build_mail_command("alice@test", false, message, &peer, None),
            MailCommand::Ready(command)
                if command == format!("MAIL FROM:<alice@test> SIZE={} BODY=BINARYMIME\r\n", message.len())
        ));
    }
}
