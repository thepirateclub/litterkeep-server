//! Проверки GS2-профиля и полей первого сообщения SCRAM клиента.
use super::*;

#[test]
fn parses_rfc_example_shape_and_retains_exact_authentication_transcript() {
    let wire = "n,,n=user,r=fyko+d2lbbFgONRv9qkxdawL";
    let parsed = parse(wire).expect("client-first message");
    assert_eq!(parsed.username, "user");
    assert_eq!(parsed.nonce, "fyko+d2lbbFgONRv9qkxdawL");
    assert_eq!(
        parsed.client_first_bare,
        "n=user,r=fyko+d2lbbFgONRv9qkxdawL"
    );
}

#[test]
fn ignores_optional_extensions_without_changing_the_authentication_transcript() {
    let parsed = parse("n,,n=user,r=clientNonce,x=optional").expect("optional extension");
    assert_eq!(parsed.username, "user");
    assert_eq!(parsed.nonce, "clientNonce");
    assert_eq!(parsed.client_first_bare, "n=user,r=clientNonce,x=optional");
}

#[test]
fn decodes_sasl_name_escapes_and_prepares_the_account_name() {
    let parsed = parse("n,,n=mail=2Cbox=3D1,r=clientNonce").expect("escaped account");
    assert_eq!(parsed.username, "mail,box=1");
}

#[test]
fn saslprep_maps_compatibility_characters_before_account_lookup() {
    let parsed = parse("n,,n=\u{00AA},r=clientNonce").expect("SASLprep query profile");
    assert_eq!(parsed.username, "a");
}

#[test]
fn rejects_authzid_channel_binding_reordered_fields_and_unknown_extensions() {
    for wire in [
        "y,,n=user,r=nonce",
        "n,a=user,n,,n=user,r=nonce",
        "n,,r=nonce,n=user",
        "n,,n=user,x=extension,r=nonce",
    ] {
        assert!(parse(wire).is_err(), "accepted {wire:?}");
    }
}

#[test]
fn rejects_empty_nonce_and_invalid_username_escapes() {
    for wire in [
        "n,,n=user,r=",
        "n,,n=user,r=space nonce",
        "n,,n=bad=2cname,r=nonce",
        "n,,n=bad=XX,r=nonce",
    ] {
        assert!(parse(wire).is_err(), "accepted {wire:?}");
    }
}
