//! Проверки route allowlist, mailbox grammar и bounded loop detector-а.

use super::redirect_policy::{Domain, Error, Recipient, hop_limit_reached};

#[test]
fn peer_route_is_typed_and_local_or_malformed_targets_are_denied() {
    let recipient = Recipient::parse("remote+tag@PEER.TEST").expect("peer route");
    assert_eq!(recipient.domain, Domain::PeerTest);
    assert_eq!(recipient.canonical(), "remote+tag@peer.test");
    assert_eq!(
        Recipient::parse("alice@litterkeep.test"),
        Err(Error::RouteDenied)
    );
    assert_eq!(
        Recipient::parse("bad..local@peer.test"),
        Err(Error::Malformed)
    );
    assert_eq!(Recipient::parse("two@@peer.test"), Err(Error::Malformed));
}

#[test]
fn eight_received_hops_stop_another_redirect() {
    let headers = (0..8)
        .map(|number| ("Received".into(), format!("by peer-{number}")))
        .collect::<Vec<_>>();
    assert!(hop_limit_reached(&headers));
    assert!(!hop_limit_reached(
        &headers.into_iter().take(7).collect::<Vec<_>>()
    ));
}
