//! Построение ESMTP MAIL FROM с согласованными параметрами расширений.

use super::{
    EhloCapabilities, EhloExtension, SizeAdvertisement,
    data::{has_8bit_body, transmitted_message_size},
};

/// Исход выбора outbound MAIL: отправить команду либо завершить job permanent.
pub(crate) enum MailCommand {
    Ready(String),
    PermanentFailure,
}

/// Создаёт MAIL FROM и объявляет только параметры, согласованные через EHLO.
pub(crate) fn build_mail_command(
    sender: &str,
    message: &[u8],
    capabilities: &EhloCapabilities,
) -> MailCommand {
    let declared_octets = transmitted_message_size(message);
    let declared_size = u64::try_from(declared_octets).unwrap_or(u64::MAX);
    let size_parameter = match capabilities.size_advertisement() {
        SizeAdvertisement::NotAdvertised => String::new(),
        SizeAdvertisement::NoFixedLimit => format!(" SIZE={declared_size}"),
        SizeAdvertisement::Maximum(maximum) if declared_size > maximum => {
            return MailCommand::PermanentFailure;
        }
        SizeAdvertisement::Maximum(_) => format!(" SIZE={declared_size}"),
    };
    let body_parameter = match has_8bit_body(message) {
        true if capabilities.supports(EhloExtension::EightBitMime) => " BODY=8BITMIME",
        true => return MailCommand::PermanentFailure,
        false => "",
    };
    MailCommand::Ready(format!(
        "MAIL FROM:<{sender}>{size_parameter}{body_parameter}\r\n"
    ))
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
            build_mail_command("alice@test", b"Subject: x\r\n\r\nbody", &peer),
            MailCommand::Ready(command) if command == "MAIL FROM:<alice@test>\r\n"
        ));
    }

    #[test]
    fn declared_size_includes_the_added_terminal_crlf_but_not_dot_stuffing() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 1024\r\n");
        let message = b"Subject: x\r\n\r\n.body";
        let expected_size = message.len().saturating_add(2);
        assert!(matches!(
            build_mail_command("alice@test", message, &peer),
            MailCommand::Ready(command)
                if command == format!("MAIL FROM:<alice@test> SIZE={expected_size}\r\n")
        ));
    }

    #[test]
    fn fixed_size_limit_rejects_before_creating_mail_command() {
        let peer = capabilities(b"250-peer.test\r\n250 SIZE 10\r\n");
        assert!(matches!(
            build_mail_command("alice@test", b"Subject: x\r\n\r\nbody", &peer),
            MailCommand::PermanentFailure
        ));
    }

    #[test]
    fn size_and_8bitmime_parameters_are_combined_in_one_mail_command() {
        let peer = capabilities(b"250-peer.test\r\n250-SIZE 1024\r\n250 8BITMIME\r\n");
        let message = "Content-Transfer-Encoding: 8bit\r\n\r\nПривет".as_bytes();
        let size = message.len().saturating_add(2);
        assert!(matches!(
            build_mail_command("alice@test", message, &peer),
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
            build_mail_command("alice@test", b"body", &peer),
            MailCommand::Ready(command) if command == "MAIL FROM:<alice@test> SIZE=6\r\n"
        ));
    }
}
