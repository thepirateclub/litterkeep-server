//! Linux wakeup-адаптер bounded outbound worker-а для poll-reactor.

use crate::outbound_worker::OutboundWorker;
use std::io;
use std::os::unix::net::UnixStream;

/// Владеет reader-концом wake socket и transport worker-ом.
///
/// Один записанный байт лишь сообщает reactor-у, что стоит вычитать completion
/// channel. При заполненном socket notifier допускает потерю байта, поскольку
/// уже доступный байт остаётся wake signal, а сами completion не теряются.
pub struct WakingOutboundWorker {
    pub worker: OutboundWorker,
    wake_reader: UnixStream,
}

impl WakingOutboundWorker {
    /// Создаёт worker и его event-driven completion wakeup в одной операции.
    pub fn start(
        address: impl Into<String>,
        queue_capacity: usize,
        pool_capacity: usize,
    ) -> io::Result<Self> {
        use std::io::Write;
        let (wake_reader, mut wake_writer) = UnixStream::pair()?;
        wake_reader.set_nonblocking(true)?;
        wake_writer.set_nonblocking(true)?;
        let worker = OutboundWorker::start_with_notifier(
            address,
            queue_capacity,
            pool_capacity,
            move || {
                let _ = wake_writer.write(&[1]);
            },
        )?;
        Ok(Self {
            worker,
            wake_reader,
        })
    }

    /// Даёт reactor-у fd, который readable после completion transport-а.
    pub fn wake_reader(&self) -> &UnixStream {
        &self.wake_reader
    }

    /// Снимает только coalesced hints, не читая authoritative completion queue.
    pub fn drain_wakeup(&mut self) -> io::Result<()> {
        use std::io::Read;
        let mut bytes = [0_u8; 256];
        loop {
            match self.wake_reader.read(&mut bytes) {
                Ok(0) => return Ok(()),
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    /// Останавливает worker до закрытия его completion wake resource.
    pub fn shutdown(self) {
        self.worker.shutdown();
    }
}
