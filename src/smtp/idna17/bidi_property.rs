//! Unicode 17 Bidi_Class lookup для IDNA2008 RFC 5893.

#[path = "bidi_ranges.rs"]
mod ranges;

/// Классы направления, необходимые RFC 5893 и точному UCD lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BidiClass {
    LeftToRight,
    RightToLeft,
    ArabicLetter,
    EuropeanNumber,
    EuropeanSeparator,
    EuropeanTerminator,
    ArabicNumber,
    CommonSeparator,
    NonspacingMark,
    BoundaryNeutral,
    ParagraphSeparator,
    SegmentSeparator,
    Whitespace,
    OtherNeutral,
    LeftToRightEmbedding,
    LeftToRightOverride,
    RightToLeftEmbedding,
    RightToLeftOverride,
    PopDirectionalFormat,
    LeftToRightIsolate,
    RightToLeftIsolate,
    FirstStrongIsolate,
    PopDirectionalIsolate,
}

/// Возвращает Bidi_Class; не перечисленные Unicode ranges имеют значение L.
pub(super) fn bidi_class(character: char) -> BidiClass {
    let code_point = u32::from(character);
    for table in ranges::TABLES {
        if let Ok(position) =
            table.binary_search_by(|(start, end, _)| compare_code_point(code_point, *start, *end))
        {
            return table
                .get(position)
                .map(|(_, _, class)| *class)
                .unwrap_or(BidiClass::LeftToRight);
        }
    }
    BidiClass::LeftToRight
}

/// Сравнивает scalar value с одним включительным диапазоном.
fn compare_code_point(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "bidi_property_tests.rs"]
mod tests;
