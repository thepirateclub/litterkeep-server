//! Общая доменная модель системных флагов сообщения.

use std::io;

/// Системные флаги, одинаковые для IMAP, store и будущего POP3 projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemFlag {
    Answered,
    Flagged,
    Deleted,
    Seen,
    Draft,
}

/// Компактный набор системных флагов одного сообщения.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MessageFlags(u8);

impl MessageFlags {
    pub fn from_flags(flags: impl IntoIterator<Item = SystemFlag>) -> Self {
        flags.into_iter().fold(Self::default(), |mut set, flag| {
            set.0 |= bit(flag);
            set
        })
    }

    pub fn contains(self, flag: SystemFlag) -> bool {
        self.0 & bit(flag) != 0
    }

    pub(crate) fn bits(self) -> u8 {
        self.0
    }

    pub(crate) fn from_bits(bits: u8) -> io::Result<Self> {
        match bits & !0b1_1111 {
            0 => Ok(Self(bits)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown flag bit",
            )),
        }
    }

    pub(crate) fn mutate(self, operation: FlagMutation, operand: Self) -> Self {
        match operation {
            FlagMutation::Replace => operand,
            FlagMutation::Add => Self(self.0 | operand.0),
            FlagMutation::Remove => Self(self.0 & !operand.0),
        }
    }
}

/// Атомарная операция над полным набором выбранных сообщений.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagMutation {
    Replace,
    Add,
    Remove,
}

fn bit(flag: SystemFlag) -> u8 {
    match flag {
        SystemFlag::Answered => 1 << 0,
        SystemFlag::Flagged => 1 << 1,
        SystemFlag::Deleted => 1 << 2,
        SystemFlag::Seen => 1 << 3,
        SystemFlag::Draft => 1 << 4,
    }
}
