//! Объединение regular и RFC 2231 continuation parameter segments.

use super::charset::Charset;
use super::language_tag::LanguageTag;

/// Один уже разобранный параметр до continuation assembly.
pub(super) struct Segment {
    index: Option<usize>,
    encoded: bool,
    value: String,
}

impl Segment {
    /// Создаёт сегмент без раскрытия его полей parser-у имени.
    pub(super) fn new(index: Option<usize>, encoded: bool, value: String) -> Self {
        Self {
            index,
            encoded,
            value,
        }
    }
}

/// Соединяет секции параметра и возвращает декодированное значение с language.
pub(super) fn join_segments(
    mut segments: Vec<Segment>,
) -> Result<(String, Option<LanguageTag>), &'static str> {
    reject_conflicting_parameter_forms(&segments)?;
    match segments.iter().all(|segment| segment.index.is_none()) {
        true if segments.len() == 1 => decode_single(segments.pop().expect("one segment")),
        true => Err("duplicate MIME parameter"),
        false => join_continuations(segments),
    }
}

/// Не допускает одновременно regular parameter и его RFC 2231-форму.
fn reject_conflicting_parameter_forms(segments: &[Segment]) -> Result<(), &'static str> {
    let has_regular = segments
        .iter()
        .any(|segment| segment.index.is_none() && !segment.encoded);
    let has_extended = segments
        .iter()
        .any(|segment| segment.index.is_some() || segment.encoded);
    match (has_regular, has_extended) {
        (true, true) => Err("conflicting regular and extended MIME parameter"),
        _ => Ok(()),
    }
}

fn decode_single(segment: Segment) -> Result<(String, Option<LanguageTag>), &'static str> {
    match segment.encoded {
        true => decode_extended(&segment.value),
        false => Ok((segment.value, None)),
    }
}

fn join_continuations(
    mut segments: Vec<Segment>,
) -> Result<(String, Option<LanguageTag>), &'static str> {
    segments.sort_by_key(|segment| segment.index);
    let mut bytes = Vec::new();
    let mut charset = None;
    let mut language = None;
    let mut charset_was_omitted = false;
    for (expected, segment) in segments.into_iter().enumerate() {
        if segment.index != Some(expected) {
            return Err("non-contiguous MIME continuation");
        }
        let decoded = match (expected, segment.encoded) {
            (0, true) => {
                let initial = super::parameter_extended::initial(&segment.value)?;
                match initial.charset {
                    Some(selected) => charset = Some(selected),
                    None => charset_was_omitted = true,
                }
                language = initial.language;
                initial.octets
            }
            (_, true) => super::parameter_extended::continuation(&segment.value)?,
            (_, false) => segment.value.into_bytes(),
        };
        bytes.extend(decoded);
    }
    Ok((
        decode_joined(charset, bytes, charset_was_omitted)?,
        language,
    ))
}

fn decode_joined(
    charset: Option<Charset>,
    bytes: Vec<u8>,
    charset_was_omitted: bool,
) -> Result<String, &'static str> {
    match charset {
        Some(selected) => selected.decode(&bytes),
        None if charset_was_omitted => decode_unlabelled_ascii(&bytes),
        None => String::from_utf8(bytes).map_err(|_| "continuation is not UTF-8"),
    }
}

fn decode_extended(value: &str) -> Result<(String, Option<LanguageTag>), &'static str> {
    let initial = super::parameter_extended::initial(value)?;
    let decoded = match initial.charset {
        Some(charset) => charset.decode(&initial.octets)?,
        None => decode_unlabelled_ascii(&initial.octets)?,
    };
    Ok((decoded, initial.language))
}

/// Charset можно опустить для ASCII payload, но не угадывать high-bit encoding.
fn decode_unlabelled_ascii(bytes: &[u8]) -> Result<String, &'static str> {
    if !bytes.is_ascii() {
        return Err("non-ASCII RFC 2231 payload has no charset label");
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "invalid ASCII RFC 2231 payload")
}
