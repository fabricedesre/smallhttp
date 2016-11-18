// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

#[derive(Debug)]
pub enum ChannelError {
    SomethingWentWrong,
    InvalidHostName,
    UnableToConnect,
    EndOfStream,
}

pub trait Channel {
    // Opens a channel to the given host:port destination, with TLS support is needed.
    fn open(&mut self, host: &str, port: u16, tls: bool) -> Result<(), ChannelError>;

    // Tries to send `len` bytes. Returns the number of bytes successfully sent,
    // or an error.
    fn send(&self, data: &[u8], len: usize) -> Result<usize, ChannelError>;

    fn send_str(&self, data: &str) -> Result<usize, ChannelError> {
        self.send(data.as_bytes(), data.len())
    }

    // Tries to receive at most `max_len` bytes. Returns the number of bytes successfully received,
    // or an error.
    fn recv(&self, data: &mut [u8], max_len: usize) -> Result<usize, ChannelError>;
}
