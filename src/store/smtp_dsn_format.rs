//! Общие constants и checksum versioned SMTP DSN frame.

pub(super) const MAGIC: [u8; 4] = *b"LKSD";
pub(super) const VERSION: u16 = 1;
pub(super) const COMMIT: u32 = 0xd50c_0de1;
pub(super) const MAX_FRAME: usize = 65_536;
pub(super) const MAX_RECIPIENTS: usize = 100;

/// FNV checksum обнаруживает повреждение полного frame до разбора payload.
pub(super) fn checksum(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut value, byte| {
            value ^= u64::from(*byte);
            value.wrapping_mul(0x100_0000_01b3)
        })
}
