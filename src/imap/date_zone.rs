//! RFC 5322 и RFC 2183 профили разбора часовых зон date-time.

/// Разделяет полный RFC 5322 Date и numeric-only RFC 2183 date-параметр.
#[derive(Clone, Copy)]
pub(super) enum DateTimeProfile {
    Rfc5322,
    Rfc2183NumericZone,
}

/// Нормализует wire zone только по явно выбранной грамматической политике.
pub(super) fn normalized_zone(value: &str, profile: DateTimeProfile) -> Option<String> {
    match value.chars().next() {
        Some('+' | '-') => Some(value.to_owned()),
        Some(_) => match profile {
            DateTimeProfile::Rfc5322 => ObsoleteZone::parse(value).map(ObsoleteZone::offset),
            DateTimeProfile::Rfc2183NumericZone => None,
        },
        None => None,
    }
}

/// Типизированные RFC 5322 §4.3 obsolete zones и безопасный fallback.
#[derive(Clone, Copy)]
enum ObsoleteZone {
    Greenwich,
    EasternStandard,
    EasternDaylight,
    CentralStandard,
    CentralDaylight,
    MountainStandard,
    MountainDaylight,
    PacificStandard,
    PacificDaylight,
    UnknownOffset,
}

impl ObsoleteZone {
    /// Преобразует zone token в enum до календарного расчёта.
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "UT" | "GMT" => Some(Self::Greenwich),
            "EST" => Some(Self::EasternStandard),
            "EDT" => Some(Self::EasternDaylight),
            "CST" => Some(Self::CentralStandard),
            "CDT" => Some(Self::CentralDaylight),
            "MST" => Some(Self::MountainStandard),
            "MDT" => Some(Self::MountainDaylight),
            "PST" => Some(Self::PacificStandard),
            "PDT" => Some(Self::PacificDaylight),
            value if is_military_zone(value) => Some(Self::UnknownOffset),
            value if is_unknown_named_zone(value) => Some(Self::UnknownOffset),
            _ => None,
        }
    }

    /// Возвращает numeric offset, применяемый общим Gregorian calendar core.
    fn offset(self) -> String {
        let offset = match self {
            Self::Greenwich => "+0000",
            Self::EasternStandard => "-0500",
            Self::EasternDaylight => "-0400",
            Self::CentralStandard => "-0600",
            Self::CentralDaylight => "-0500",
            Self::MountainStandard => "-0700",
            Self::MountainDaylight => "-0600",
            Self::PacificStandard => "-0800",
            Self::PacificDaylight => "-0700",
            // Неопределённые military и исторические зоны сравниваются как -0000.
            Self::UnknownOffset => "-0000",
        };
        offset.to_owned()
    }
}

/// Разбирает военную зону, исключая зарезервированную букву J.
fn is_military_zone(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(
        (characters.next(), characters.next()),
        (Some('A'..='I' | 'K'..='Z'), None)
    )
}

/// Узнаёт 3-5-буквенные исторические зоны с неизвестным UTC offset.
fn is_unknown_named_zone(value: &str) -> bool {
    matches!(
        (
            value.len(),
            value.bytes().all(|octet| octet.is_ascii_alphabetic())
        ),
        (3..=5, true)
    )
}
