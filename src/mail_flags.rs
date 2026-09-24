//! Общая доменная модель системных флагов сообщения.

use std::{collections::BTreeSet, io};

/// Пользовательское IMAP keyword в нормализованном регистре.
///
/// В отличие от закрытого набора системных флагов keyword расширяем и поэтому
/// хранится как проверенный atom, а не как новый вариант SystemFlag.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeywordFlag(String);

impl KeywordFlag {
    /// Создаёт keyword только из допустимых ASCII atom characters.
    pub fn parse(value: &str) -> Option<Self> {
        let valid = value.bytes().all(|byte| match byte {
            0x21..=0x7e => !matches!(byte, b'(' | b')' | b'{' | b'%' | b'*' | b'"' | b'\\' | b']'),
            _ => false,
        });
        match (value.is_empty(), value.starts_with('\\'), valid) {
            (false, false, true) => Some(Self(value.to_ascii_uppercase())),
            _ => None,
        }
    }

    /// Возвращает канонический atom для SEARCH и FETCH FLAGS.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Полное состояние persistent flags одного сообщения.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredFlags {
    pub system: MessageFlags,
    pub keywords: BTreeSet<KeywordFlag>,
}

impl StoredFlags {
    /// Применяет одну операцию к системным флагам и, если переданы,
    /// пользовательским keywords. Внутренние операции могут передать None,
    /// чтобы, например, установка `\\Seen` сохранила весь keyword set.
    pub(crate) fn apply(
        mut self,
        operation: FlagMutation,
        system: MessageFlags,
        keywords: Option<&BTreeSet<KeywordFlag>>,
    ) -> Self {
        self.system = self.system.mutate(operation, system);
        if let Some(keywords) = keywords {
            match operation {
                FlagMutation::Replace => self.keywords.clone_from(keywords),
                FlagMutation::Add => self.keywords.extend(keywords.iter().cloned()),
                FlagMutation::Remove => self.keywords.retain(|item| !keywords.contains(item)),
            }
        }
        self
    }
}

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

#[cfg(test)]
mod keyword_tests {
    use super::KeywordFlag;

    #[test]
    fn keyword_is_case_insensitive_and_accepts_rfc_atom_characters() {
        let keyword = KeywordFlag::parse("$Forwarded").expect("RFC keyword");
        assert_eq!(keyword.as_str(), "$FORWARDED");
        assert!(KeywordFlag::parse("project-2026").is_some());
    }

    #[test]
    fn keyword_rejects_system_flag_prefix_and_atom_delimiters() {
        for value in [
            "",
            "\\Seen",
            "bad value",
            "(bad)",
            "bad*",
            "bad\\flag",
            "bad]",
        ] {
            assert!(KeywordFlag::parse(value).is_none(), "{value:?}");
        }
    }
}
