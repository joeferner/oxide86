use std::collections::VecDeque;

pub enum TransportEvent {
    /// TCP connect succeeded.
    Connected,
    /// TCP connect failed.
    ConnectFailed,
    /// Remote peer closed the connection.
    RemoteDisconnected,
}

/// Platform-specific connection to a remote host, used by `SerialModem`.
pub trait ModemTransport: Send + Sync {
    /// Drain all buffered incoming bytes.
    fn poll_incoming(&mut self, out: &mut VecDeque<u8>);
    /// Send one byte to the remote.
    fn send_byte(&mut self, byte: u8);
    /// Return the next pending state-change event (at most once per state).
    fn take_event(&mut self) -> Option<TransportEvent>;
    /// Start the wall-clock escape guard. `guard_ms` is the timeout in milliseconds.
    fn start_escape_guard(&mut self, guard_ms: u64);
    /// Cancel/reset the wall-clock escape guard.
    fn cancel_escape_guard(&mut self);
    /// True if the wall-clock escape guard has elapsed.
    fn escape_time_elapsed(&self) -> bool;
}

/// Factory that creates a `ModemTransport` for a given dial address.
pub trait ModemDialer: Send + Sync {
    fn dial(&self, addr: &str) -> Box<dyn ModemTransport>;
}
