//! Проверка грамматической границы MIME parameter вокруг знака равенства.

/// RFC 2045 §5.1 и RFC 2231 §7 требуют смежности имени, `=` и значения.
///
/// Комментарии предварительно удаляются в режиме сохранения соседства,
/// поэтому допустимый RFC 822 comment сам по себе не принимается за пробел.
pub(super) fn validate(section: &str) -> Result<(), &'static str> {
    let uncommented = super::structured_comments::remove_preserving_adjacency(section)?;
    let (name, value) = uncommented
        .split_once('=')
        .ok_or("parameter without value")?;
    match (name.ends_with([' ', '\t']), value.starts_with([' ', '\t'])) {
        (false, false) => Ok(()),
        _ => Err("whitespace around MIME parameter equals"),
    }
}
