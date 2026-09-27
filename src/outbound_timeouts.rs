//! Нормативные per-command таймауты исходящего SMTP-клиента.

use super::ReplyExpectation;
use std::{env, ffi::OsString, io, time::Duration};

/// Таймауты отдельной SMTP-фазы и передачи DATA.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SmtpTimeouts {
    greeting: Duration,
    command: Duration,
    data_initiation: Duration,
    data_block: Duration,
    data_termination: Duration,
}

impl SmtpTimeouts {
    /// Создаёт одинаковые интервалы только для ограниченных тестовых peer-ов.
    #[cfg(test)]
    pub(crate) fn uniform(interval: Duration) -> Self {
        Self {
            greeting: interval,
            command: interval,
            data_initiation: interval,
            data_block: interval,
            data_termination: interval,
        }
    }

    /// Создаёт тестовую пару с длинным greeting и коротким обычным deadline.
    #[cfg(test)]
    pub(crate) fn greeting_and_command(greeting: Duration, command: Duration) -> Self {
        Self {
            greeting,
            command,
            ..Self::default()
        }
    }

    /// Задаёт короткие интервалы для socket-тестов промежуточного BDAT и LAST.
    #[cfg(test)]
    pub(crate) fn bdat_phases(command: Duration, termination: Duration) -> Self {
        Self {
            command,
            data_termination: termination,
            ..Self::default()
        }
    }

    /// Выбирает ожидание ответа по текущей SMTP-команде.
    pub(crate) fn response(self, expectation: ReplyExpectation) -> Duration {
        match expectation {
            ReplyExpectation::Greeting => self.greeting,
            ReplyExpectation::Data => self.data_initiation,
            ReplyExpectation::Message | ReplyExpectation::BdatLast => self.data_termination,
            ReplyExpectation::Bdat => self.command,
            _ => self.command,
        }
    }

    /// Возвращает предел ожидания завершения каждой системной отправки DATA.
    pub(crate) fn data_block(self) -> Duration {
        self.data_block
    }

    /// Возвращает предел ожидания записи отдельной SMTP-команды.
    pub(crate) fn command(self) -> Duration {
        self.command
    }

    /// Возвращает предел ожидания initial greeting до обработки EHLO.
    pub(crate) fn greeting(self) -> Duration {
        self.greeting
    }

    /// Читает phase timeouts из environment либо применяет RFC-значения.
    pub(crate) fn from_environment() -> io::Result<Self> {
        Self::from_lookup(&mut |setting| env::var_os(setting.name()))
    }

    fn from_lookup(
        lookup: &mut impl FnMut(TimeoutSetting) -> Option<OsString>,
    ) -> io::Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            greeting: read_seconds(TimeoutSetting::Greeting, defaults.greeting, lookup)?,
            command: read_seconds(TimeoutSetting::Command, defaults.command, lookup)?,
            data_initiation: read_seconds(
                TimeoutSetting::DataInitiation,
                defaults.data_initiation,
                lookup,
            )?,
            data_block: read_seconds(TimeoutSetting::DataBlock, defaults.data_block, lookup)?,
            data_termination: read_seconds(
                TimeoutSetting::DataTermination,
                defaults.data_termination,
                lookup,
            )?,
        })
    }
}

/// Типизированный ключ пользовательской настройки одной SMTP-фазы.
#[derive(Clone, Copy)]
enum TimeoutSetting {
    Greeting,
    Command,
    DataInitiation,
    DataBlock,
    DataTermination,
}

impl TimeoutSetting {
    /// Возвращает публичное имя настройки для стандартного process environment.
    fn name(self) -> &'static str {
        match self {
            Self::Greeting => "LITTERKEEP_SMTP_GREETING_TIMEOUT_SECONDS",
            Self::Command => "LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS",
            Self::DataInitiation => "LITTERKEEP_SMTP_DATA_START_TIMEOUT_SECONDS",
            Self::DataBlock => "LITTERKEEP_SMTP_DATA_BLOCK_TIMEOUT_SECONDS",
            Self::DataTermination => "LITTERKEEP_SMTP_DATA_FINAL_TIMEOUT_SECONDS",
        }
    }
}

fn read_seconds(
    setting: TimeoutSetting,
    default: Duration,
    lookup: &mut impl FnMut(TimeoutSetting) -> Option<OsString>,
) -> io::Result<Duration> {
    let Some(value) = lookup(setting) else {
        return Ok(default);
    };
    let parsed = value
        .to_str()
        .and_then(|text| text.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} must be a positive integer number of seconds",
                    setting.name()
                ),
            )
        })?;
    Ok(Duration::from_secs(parsed))
}

impl Default for SmtpTimeouts {
    /// Значения соответствуют рекомендуемым минимумам RFC 5321 §4.5.3.2.
    fn default() -> Self {
        Self {
            greeting: Duration::from_secs(5 * 60),
            command: Duration::from_secs(5 * 60),
            data_initiation: Duration::from_secs(2 * 60),
            data_block: Duration::from_secs(3 * 60),
            data_termination: Duration::from_secs(10 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SmtpTimeouts, TimeoutSetting};
    use crate::outbound::ReplyExpectation;
    use std::ffi::OsString;
    use std::time::Duration;

    #[test]
    fn reply_timeout_is_selected_by_smtp_phase() {
        let timeouts = SmtpTimeouts::default();
        assert_eq!(
            timeouts.response(ReplyExpectation::Greeting),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Mail),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Recipient),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Data),
            Duration::from_secs(120)
        );
        assert_eq!(timeouts.data_block(), Duration::from_secs(180));
        assert_eq!(
            timeouts.response(ReplyExpectation::Message),
            Duration::from_secs(600)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Bdat),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::BdatLast),
            Duration::from_secs(600)
        );
    }

    #[test]
    fn environment_overrides_are_applied_per_timeout_setting() {
        let timeouts = SmtpTimeouts::from_lookup(&mut |setting| match setting {
            TimeoutSetting::Greeting => Some(OsString::from("41")),
            TimeoutSetting::DataBlock => Some(OsString::from("73")),
            _ => None,
        })
        .expect("timeouts");
        assert_eq!(
            timeouts.response(ReplyExpectation::Greeting),
            Duration::from_secs(41)
        );
        assert_eq!(timeouts.command(), Duration::from_secs(300));
        assert_eq!(timeouts.data_block(), Duration::from_secs(73));
    }

    #[test]
    fn invalid_environment_timeout_fails_configuration_validation() {
        let error = SmtpTimeouts::from_lookup(&mut |setting| match setting {
            TimeoutSetting::Command => Some(OsString::from("0")),
            _ => None,
        })
        .expect_err("zero timeout");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS")
        );
    }
}
