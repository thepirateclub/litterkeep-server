//! Ограниченное накопление и потоковое отбрасывание POP3 SASL-строк.

use super::MAX_INPUT;

/// Результат добавления данных в буфер POP3 session.
pub(super) enum InputProgress {
    /// Данные накоплены в пределах обычного входного лимита.
    Appended,
    /// Длинная SASL-строка отбрасывается до её CRLF без роста памяти.
    Discarding,
    /// Граница длинной SASL-строки найдена, session готова к ошибочному ответу.
    OversizedComplete,
}

/// Владение границей одной слишком длинной SASL continuation строки.
#[derive(Default)]
pub(super) struct Pop3SaslInput {
    discarding: bool,
    previous_was_carriage_return: bool,
}

impl Pop3SaslInput {
    /// Накопляет обычные данные либо ограниченно удаляет слишком длинный ответ.
    pub(super) fn append(
        &mut self,
        input: &mut Vec<u8>,
        received: &[u8],
        awaiting_sasl_response: bool,
    ) -> InputProgress {
        match self.discarding {
            true => self.discard_until_crlf(input, received),
            false => self.append_with_limit(input, received, awaiting_sasl_response),
        }
    }

    fn append_with_limit(
        &mut self,
        input: &mut Vec<u8>,
        received: &[u8],
        awaiting_sasl_response: bool,
    ) -> InputProgress {
        input.extend_from_slice(received);
        match (awaiting_sasl_response, input.len() > MAX_INPUT) {
            (true, true) => self.begin_discard(input),
            _ => InputProgress::Appended,
        }
    }

    fn begin_discard(&mut self, input: &mut Vec<u8>) -> InputProgress {
        match input.windows(2).position(|pair| pair == b"\r\n") {
            Some(end) => self.finish_complete_line(input, end),
            None => {
                self.discarding = true;
                self.previous_was_carriage_return = input.last() == Some(&b'\r');
                input.clear();
                InputProgress::Discarding
            }
        }
    }

    fn finish_complete_line(&self, input: &mut Vec<u8>, line_end: usize) -> InputProgress {
        let line_size = line_end.saturating_add(2);
        match line_size > MAX_INPUT {
            true => {
                let _ = input.drain(..line_size);
                InputProgress::OversizedComplete
            }
            false => InputProgress::Appended,
        }
    }

    fn discard_until_crlf(&mut self, input: &mut Vec<u8>, received: &[u8]) -> InputProgress {
        let mut previous_was_carriage_return = self.previous_was_carriage_return;
        let mut end = None;
        for (offset, octet) in received.iter().copied().enumerate() {
            match (octet, previous_was_carriage_return) {
                (b'\n', true) => {
                    end = Some(offset);
                    break;
                }
                _ => previous_was_carriage_return = octet == b'\r',
            }
        }
        match end {
            Some(offset) => self.finish_discard(input, received, offset),
            None => {
                self.previous_was_carriage_return = previous_was_carriage_return;
                InputProgress::Discarding
            }
        }
    }

    fn finish_discard(
        &mut self,
        input: &mut Vec<u8>,
        received: &[u8],
        line_end: usize,
    ) -> InputProgress {
        self.discarding = false;
        self.previous_was_carriage_return = false;
        let remainder_start = line_end.saturating_add(1);
        if let Some(remainder) = received.get(remainder_start..) {
            input.extend_from_slice(remainder);
        }
        InputProgress::OversizedComplete
    }
}

#[cfg(test)]
#[path = "pop3_sasl_input_tests.rs"]
mod tests;
