//! Ограниченный Punycode codec по RFC 3492 без зависимости от сторонних crates.

const BASE: u64 = 36;
const TMIN: u64 = 1;
const TMAX: u64 = 26;
const SKEW: u64 = 38;
const DAMP: u64 = 700;
const INITIAL_BIAS: u64 = 72;
const INITIAL_N: u64 = 128;
const DELIMITER: u8 = b'-';
const MAX_INPUT_BYTES: usize = 1024;

/// Ошибка Punycode codec без детализации пользовательских адресных данных.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Вход превышает локальный предел, защищающий codec от неограниченной работы.
    InputTooLong,
    /// Вход не является корректным ASCII Punycode либо Unicode scalar sequence.
    InvalidInput,
    /// Промежуточная арифметика выходит за representable диапазон.
    Overflow,
}

/// Кодирует Unicode code points в Punycode, не добавляя IDNA ACE prefix.
///
/// Функция реализует только RFC 3492 Bootstring. Проверку U-label по RFC 5890,
/// IDNA contextual/bidi rules и предел DNS label обязан выполнить вызывающий слой.
pub fn encode(input: &str) -> Result<String, Error> {
    check_input_size(input.len())?;
    let points = input.chars().map(u32::from).collect::<Vec<_>>();
    let mut output = String::with_capacity(input.len());
    let basic_count = copy_basic_points(&points, &mut output);
    let mut handled = basic_count;
    let mut n = INITIAL_N;
    let mut delta = 0_u64;
    let mut bias = INITIAL_BIAS;
    append_delimiter_after_basic_points(basic_count, &mut output);
    while handled < points.len() {
        let next = minimum_unhandled_point(&points, n)?;
        delta = advance_delta(delta, next - n, handled)?;
        n = next;
        for point in points.iter().copied() {
            let point = u64::from(point);
            match point.cmp(&n) {
                std::cmp::Ordering::Less => delta = delta.checked_add(1).ok_or(Error::Overflow)?,
                std::cmp::Ordering::Equal => {
                    encode_delta(delta, bias, &mut output)?;
                    let point_count = u64::try_from(handled.checked_add(1).ok_or(Error::Overflow)?)
                        .map_err(|_| Error::Overflow)?;
                    bias = adapt(delta, point_count, handled == basic_count)?;
                    delta = 0;
                    handled = handled.checked_add(1).ok_or(Error::Overflow)?;
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        delta = delta.checked_add(1).ok_or(Error::Overflow)?;
        n = n.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok(output)
}

/// Декодирует ASCII Punycode body, не проверяя его применимость как IDNA A-label.
pub fn decode(input: &str) -> Result<String, Error> {
    check_input_size(input.len())?;
    if !input.is_ascii() {
        return Err(Error::InvalidInput);
    }
    let bytes = input.as_bytes();
    let delimiter = bytes.iter().rposition(|byte| *byte == DELIMITER);
    let (basic, encoded_start) = match delimiter {
        Some(position) => (
            bytes.get(..position).ok_or(Error::InvalidInput)?,
            position.saturating_add(1),
        ),
        None => (&[][..], 0),
    };
    let mut output = basic
        .iter()
        .map(|byte| u32::from(*byte))
        .collect::<Vec<_>>();
    let mut cursor = encoded_start;
    let mut n = INITIAL_N;
    let mut insertion = 0_u64;
    let mut bias = INITIAL_BIAS;
    while cursor < bytes.len() {
        let old_insertion = insertion;
        let (decoded_delta, next_cursor) = decode_delta(bytes, cursor, bias)?;
        cursor = next_cursor;
        let output_len = u64::try_from(output.len()).map_err(|_| Error::Overflow)?;
        let divisor = output_len.checked_add(1).ok_or(Error::Overflow)?;
        insertion = insertion
            .checked_add(decoded_delta)
            .ok_or(Error::Overflow)?;
        bias = adapt(
            insertion
                .checked_sub(old_insertion)
                .ok_or(Error::Overflow)?,
            divisor,
            old_insertion == 0,
        )?;
        n = n.checked_add(insertion / divisor).ok_or(Error::Overflow)?;
        insertion %= divisor;
        let scalar = char::from_u32(u32::try_from(n).map_err(|_| Error::Overflow)?)
            .ok_or(Error::InvalidInput)?;
        insert_scalar(
            &mut output,
            usize::try_from(insertion).map_err(|_| Error::Overflow)?,
            scalar,
        )?;
        insertion = insertion.checked_add(1).ok_or(Error::Overflow)?;
    }
    output
        .into_iter()
        .try_fold(String::new(), |mut text, point| {
            text.push(char::from_u32(point).ok_or(Error::InvalidInput)?);
            Ok(text)
        })
}

/// Ограничивает память и время даже при прямом вызове codec вне SMTP parser-а.
fn check_input_size(size: usize) -> Result<(), Error> {
    match size <= MAX_INPUT_BYTES {
        true => Ok(()),
        false => Err(Error::InputTooLong),
    }
}

/// Переносит ASCII code points и возвращает их количество для Bootstring state.
fn copy_basic_points(points: &[u32], output: &mut String) -> usize {
    let mut count = 0_usize;
    for point in points
        .iter()
        .copied()
        .filter(|point| *point < INITIAL_N as u32)
    {
        if let Some(character) = char::from_u32(point) {
            output.push(character);
            count = count.saturating_add(1);
        }
    }
    count
}

/// Ставит delimiter после basic code points согласно RFC 3492 §6.3.
fn append_delimiter_after_basic_points(basic: usize, output: &mut String) {
    if basic > 0 {
        output.push(char::from(DELIMITER));
    }
}

/// Находит следующий минимальный code point без индексного доступа к массиву.
fn minimum_unhandled_point(points: &[u32], n: u64) -> Result<u64, Error> {
    points
        .iter()
        .copied()
        .map(u64::from)
        .filter(|point| *point >= n)
        .min()
        .ok_or(Error::InvalidInput)
}

/// Учитывает скачок к следующему code point с проверкой переполнения.
fn advance_delta(delta: u64, distance: u64, handled: usize) -> Result<u64, Error> {
    let weight = u64::try_from(handled)
        .map_err(|_| Error::Overflow)?
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    distance
        .checked_mul(weight)
        .and_then(|step| delta.checked_add(step))
        .ok_or(Error::Overflow)
}

/// Кодирует одну delta generalized variable-length digits.
fn encode_delta(mut delta: u64, bias: u64, output: &mut String) -> Result<(), Error> {
    let mut threshold_index = BASE;
    loop {
        let threshold = threshold(bias, threshold_index);
        if delta < threshold {
            break;
        }
        let digit = threshold + (delta - threshold) % (BASE - threshold);
        output.push(char::from(encode_digit(digit)?));
        delta = (delta - threshold) / (BASE - threshold);
        threshold_index = threshold_index.checked_add(BASE).ok_or(Error::Overflow)?;
    }
    output.push(char::from(encode_digit(delta)?));
    Ok(())
}

/// Декодирует одну generalized variable-length delta и возвращает следующий cursor.
fn decode_delta(bytes: &[u8], start: usize, bias: u64) -> Result<(u64, usize), Error> {
    let mut cursor = start;
    let mut threshold_index = BASE;
    let mut weight = 1_u64;
    let mut value = 0_u64;
    loop {
        let byte = bytes.get(cursor).copied().ok_or(Error::InvalidInput)?;
        cursor = cursor.checked_add(1).ok_or(Error::Overflow)?;
        let digit = u64::from(decode_digit(byte).ok_or(Error::InvalidInput)?);
        value = digit
            .checked_mul(weight)
            .and_then(|part| value.checked_add(part))
            .ok_or(Error::Overflow)?;
        let boundary = threshold(bias, threshold_index);
        if digit < boundary {
            return Ok((value, cursor));
        }
        weight = weight.checked_mul(BASE - boundary).ok_or(Error::Overflow)?;
        threshold_index = threshold_index.checked_add(BASE).ok_or(Error::Overflow)?;
    }
}

/// Пересчитывает bias по RFC 3492 §6.1.
fn adapt(delta: u64, points: u64, first: bool) -> Result<u64, Error> {
    let divisor = match first {
        true => DAMP,
        false => 2,
    };
    let mut scaled = delta
        .checked_div(divisor)
        .and_then(|value| value.checked_add(value / points))
        .ok_or(Error::Overflow)?;
    let mut k = 0_u64;
    while scaled > ((BASE - TMIN) * TMAX) / 2 {
        scaled /= BASE - TMIN;
        k = k.checked_add(BASE).ok_or(Error::Overflow)?;
    }
    let numerator = (BASE - TMIN + 1)
        .checked_mul(scaled)
        .ok_or(Error::Overflow)?;
    let denominator = scaled.checked_add(SKEW).ok_or(Error::Overflow)?;
    k.checked_add(numerator / denominator)
        .ok_or(Error::Overflow)
}

/// Выбирает RFC threshold по текущему bias и позиции generalized integer.
fn threshold(bias: u64, k: u64) -> u64 {
    match k {
        _ if k <= bias.saturating_add(TMIN) => TMIN,
        _ if k >= bias.saturating_add(TMAX) => TMAX,
        _ => k.saturating_sub(bias),
    }
}

/// Преобразует значение digit в ASCII букву или цифру.
fn encode_digit(digit: u64) -> Result<u8, Error> {
    match digit {
        0..=25 => u8::try_from(digit)
            .map(|value| value.saturating_add(b'a'))
            .map_err(|_| Error::Overflow),
        26..=35 => u8::try_from(digit - 26)
            .map(|value| value.saturating_add(b'0'))
            .map_err(|_| Error::Overflow),
        _ => Err(Error::InvalidInput),
    }
}

/// Принимает ASCII digit в обоих регистрах, как требует RFC 3492 decoder.
fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'a'..=b'z' => Some(byte - b'a'),
        b'A'..=b'Z' => Some(byte - b'A'),
        b'0'..=b'9' => Some(byte - b'0' + 26),
        _ => None,
    }
}

/// Вставляет scalar в вычисленную позицию, отвергая позицию за концом строки.
fn insert_scalar(output: &mut Vec<u32>, position: usize, scalar: char) -> Result<(), Error> {
    match position <= output.len() {
        true => {
            output.insert(position, u32::from(scalar));
            Ok(())
        }
        false => Err(Error::InvalidInput),
    }
}

#[cfg(test)]
#[path = "punycode_tests.rs"]
mod tests;
