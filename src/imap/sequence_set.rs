//! Компактное представление IMAP sequence-set без разворачивания диапазонов.
//!
//! Вход `1:4294967295` занимает одну запись, а не миллиарды элементов. Набор
//! разрешается только относительно снимка выбранного mailbox, поэтому `*`
//! всегда означает максимум именно этого снимка.

/// Один конец диапазона IMAP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Endpoint {
    Number(u32),
    Last,
}

/// Одиночный номер либо включительный диапазон независимо от порядка концов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Range {
    start: Endpoint,
    end: Endpoint,
}

/// Ограниченный синтаксический набор номеров сообщений или UID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceSet {
    ranges: Vec<Range>,
}

/// Нормализованный снимок диапазонов для многократного быстрого поиска.
pub struct ResolvedSequenceSet {
    ranges: Vec<(u64, u64)>,
}

impl SequenceSet {
    /// Разбирает список диапазонов, не материализуя отдельные номера.
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        const MAX_RANGES: usize = 64;
        let mut ranges = Vec::new();
        for item in input.split(',') {
            if ranges.len() >= MAX_RANGES {
                return Err("too many sequence ranges");
            }
            let mut endpoints = item.split(':');
            let start = parse_endpoint(endpoints.next().unwrap_or(""))?;
            let end = match endpoints.next() {
                Some(value) => parse_endpoint(value)?,
                None => start,
            };
            if endpoints.next().is_some() {
                return Err("invalid sequence range");
            }
            ranges.push(Range { start, end });
        }
        if ranges.is_empty() {
            return Err("empty sequence set");
        }
        Ok(Self { ranges })
    }

    /// Один раз разрешает `*`, сортирует и объединяет пересечения диапазонов.
    pub fn resolve(&self, maximum: u64) -> ResolvedSequenceSet {
        let mut resolved: Vec<(u64, u64)> = self
            .ranges
            .iter()
            .map(|range| {
                let start = resolve_endpoint(range.start, maximum);
                let end = resolve_endpoint(range.end, maximum);
                (start.min(end), start.max(end))
            })
            .collect();
        resolved.sort_unstable_by_key(|range| range.0);
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(resolved.len());
        for current in resolved {
            match merged.last_mut() {
                Some(previous) if current.0 <= previous.1.saturating_add(1) => {
                    previous.1 = previous.1.max(current.1);
                }
                _ => merged.push(current),
            }
        }
        ResolvedSequenceSet { ranges: merged }
    }
}

impl ResolvedSequenceSet {
    /// Ищет номер бинарно за `O(log ranges)` без прямого обращения к индексам.
    pub fn contains(&self, value: u64) -> bool {
        self.ranges
            .binary_search_by(|range| match (value < range.0, value > range.1) {
                (true, _) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => std::cmp::Ordering::Equal,
            })
            .is_ok()
    }
}

fn parse_endpoint(input: &str) -> Result<Endpoint, &'static str> {
    match input {
        "*" => Ok(Endpoint::Last),
        value => value
            .parse::<u32>()
            .ok()
            .filter(|number| *number != 0)
            .map(Endpoint::Number)
            .ok_or("invalid sequence number"),
    }
}

fn resolve_endpoint(endpoint: Endpoint, maximum: u64) -> u64 {
    match endpoint {
        Endpoint::Number(value) => u64::from(value),
        Endpoint::Last => maximum,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Диапазоны разрешаются независимо от порядка и не дублируют результат.
    #[test]
    fn resolves_reverse_ranges_and_last() {
        let set = SequenceSet::parse("2,7:4,*").expect("sequence set");
        let resolved = set.resolve(9);
        let selected: Vec<u64> = (1..=9).filter(|value| resolved.contains(*value)).collect();
        assert_eq!(selected, vec![2, 4, 5, 6, 7, 9]);
    }

    /// Пересекающиеся и соседние диапазоны сливаются без изменения множества.
    #[test]
    fn coalesces_ranges_before_lookup() {
        let set = SequenceSet::parse("9:7,2:5,4:8").expect("sequence set");
        let resolved = set.resolve(20);
        assert!(!resolved.contains(1));
        assert!(resolved.contains(2));
        assert!(resolved.contains(9));
        assert!(!resolved.contains(10));
        assert_eq!(resolved.ranges, vec![(2, 9)]);
    }

    /// Ноль, пустые элементы и лишняя граница не являются nz-number.
    #[test]
    fn rejects_invalid_numbers() {
        for invalid in ["", "0", "1,,2", "1:2:3", "4294967296"] {
            assert!(SequenceSet::parse(invalid).is_err(), "accepted {invalid}");
        }
    }
}
