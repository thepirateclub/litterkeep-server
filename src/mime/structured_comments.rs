//! Удаление RFC 822 comments из MIME structured header values.

/// Удаляет вложенные comments, сохраняя quoted-string и экранирование.
///
/// Пробелы на границах комментария предотвращают склеивание соседних token-ов.
/// Незакрытый comment является синтаксической ошибкой structured value.
pub(crate) fn remove(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    let mut state = CommentState::Outside;
    for character in value.chars() {
        state = match (state, character) {
            (CommentState::Outside, '(') => {
                output.push(' ');
                CommentState::Inside {
                    depth: 1,
                    escaped: false,
                }
            }
            (CommentState::Outside, '"') => {
                output.push(character);
                CommentState::Quoted { escaped: false }
            }
            (CommentState::Outside, _) => {
                output.push(character);
                CommentState::Outside
            }
            (CommentState::Quoted { escaped: true }, _) => {
                output.push(character);
                CommentState::Quoted { escaped: false }
            }
            (CommentState::Quoted { escaped: false }, '\\') => {
                output.push(character);
                CommentState::Quoted { escaped: true }
            }
            (CommentState::Quoted { escaped: false }, '"') => {
                output.push(character);
                CommentState::Outside
            }
            (CommentState::Quoted { escaped: false }, _) => {
                output.push(character);
                CommentState::Quoted { escaped: false }
            }
            (
                CommentState::Inside {
                    depth,
                    escaped: true,
                },
                _,
            ) => CommentState::Inside {
                depth,
                escaped: false,
            },
            (
                CommentState::Inside {
                    depth,
                    escaped: false,
                },
                '\\',
            ) => CommentState::Inside {
                depth,
                escaped: true,
            },
            (
                CommentState::Inside {
                    depth,
                    escaped: false,
                },
                '(',
            ) => CommentState::Inside {
                depth: depth.saturating_add(1),
                escaped: false,
            },
            (
                CommentState::Inside {
                    depth,
                    escaped: false,
                },
                ')',
            ) => match depth.saturating_sub(1) {
                0 => {
                    output.push(' ');
                    CommentState::Outside
                }
                remaining => CommentState::Inside {
                    depth: remaining,
                    escaped: false,
                },
            },
            (
                CommentState::Inside {
                    depth,
                    escaped: false,
                },
                _,
            ) => CommentState::Inside {
                depth,
                escaped: false,
            },
        };
    }
    match state {
        CommentState::Outside | CommentState::Quoted { .. } => Ok(output),
        CommentState::Inside { .. } => Err("unterminated structured header comment"),
    }
}

#[derive(Clone, Copy)]
enum CommentState {
    Outside,
    Quoted { escaped: bool },
    Inside { depth: usize, escaped: bool },
}
