//! Ограниченное накопление и потоковое отбрасывание POP3 SASL-строк.

/// Результат добавления данных в буфер POP3 session.
pub(super) enum InputProgress {
    /// Данные SASL ответа сохранены в пределах доступной памяти.
    Appended,
    /// При нехватке памяти остаток одной SASL строки отбрасывается до CRLF.
    Discarding,
    /// Входная строка завершилась после отказа выделения памяти.
    AllocationFailureComplete,
}

/// Владение границей одной слишком длинной SASL continuation строки.
#[derive(Default)]
pub(super) struct Pop3SaslInput {
    discarding: bool,
    previous_was_carriage_return: bool,
}

impl Pop3SaslInput {
    /// Накопляет SASL ответ без фиксированного лимита или безопасно удаляет его.
    pub(super) fn append(
        &mut self,
        input: &mut Vec<u8>,
        received: &[u8],
        awaiting_sasl_response: bool,
    ) -> InputProgress {
        match self.discarding {
            true => self.discard_until_crlf(input, received),
            false => self.append_response(input, received, awaiting_sasl_response),
        }
    }

    fn append_response(
        &mut self,
        input: &mut Vec<u8>,
        received: &[u8],
        awaiting_sasl_response: bool,
    ) -> InputProgress {
        if !awaiting_sasl_response {
            input.extend_from_slice(received);
            return InputProgress::Appended;
        }
        match input.try_reserve(received.len()) {
            Ok(()) => {
                input.extend_from_slice(received);
                InputProgress::Appended
            }
            Err(_) => self.begin_discard(input, received),
        }
    }

    fn begin_discard(&mut self, input: &mut Vec<u8>, received: &[u8]) -> InputProgress {
        self.discarding = true;
        self.previous_was_carriage_return = input.last() == Some(&b'\r');
        *input = Vec::new();
        self.discard_until_crlf(input, received)
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
        if let Some(remainder) = received.get(remainder_start..)
            && input.try_reserve(remainder.len()).is_ok()
        {
            input.extend_from_slice(remainder);
        }
        InputProgress::AllocationFailureComplete
    }
}

#[cfg(test)]
#[path = "pop3_sasl_input_tests.rs"]
mod tests;
