// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use core::str;

#[derive(Debug, PartialEq)]
pub enum ChannelError {
    SomethingWentWrong,
    InvalidHostName,
    UnableToConnect,
    EndOfStream,
    BufferFull,
    InvalidDelimiterChar,
    InvalidString,
    TlsUnsupported,
}

pub trait Channel {
    // Opens a channel to the given host:port destination, with TLS support if needed.
    fn open(&mut self, host: &str, port: u16, tls: bool) -> Result<(), ChannelError>;

    // Tries to send `len` bytes.
    // Returns the number of bytes successfully sent, or an error.
    fn send(&mut self, data: &[u8], len: usize) -> Result<usize, ChannelError>;

    fn send_str(&mut self, data: &str) -> Result<usize, ChannelError> {
        self.send(data.as_bytes(), data.len())
    }

    // Tries to receive at most `max_len` bytes.
    // Returns the number of bytes successfully received, or an error.
    fn recv(&mut self, data: &mut [u8], max_len: usize) -> Result<usize, ChannelError>;

    // Reads data in the buffer until eof of the end of the buffer.
    fn read_to_end(&mut self, data: &mut [u8], max_len: usize) -> Result<usize, ChannelError> {
        let mut i = 0;
        let mut next = [0u8];
        while i < max_len {
            let read = self.recv(&mut next, 1);
            if let Some(err) = read.err() {
                // Convert an eof into a success returnning the number of read bytes.
                if err == ChannelError::EndOfStream {
                    return Ok(i);
                } else {
                    return Err(err);
                }
            }
            data[i] = next[0];
            i += 1;
        }
        Ok(i)
    }

    // Reads a string in the buffer until eof of the end of the buffer.
    fn read_string_to_end<'a>(&'a mut self, data: &'a mut [u8]) -> Result<&str, ChannelError> {
        let max_len = data.len();
        let size = self.read_to_end(data, max_len)?;
        let res = str::from_utf8(&data[0..size]);
        if res.is_err() {
            return Err(ChannelError::InvalidString);
        }
        return Ok(res.unwrap());
    }

    // Reads a string into a buffer, wating for a given delimiter.
    // Returns an error if we run out of buffer space or reach eof.
    // This function doesn't over-read data from the channel, but will consume the delimiter.
    fn read_string_until<'a>(&'a mut self,
                             data: &'a mut [u8],
                             delim: &str)
                             -> Result<&str, ChannelError> {
        let max_len = data.len();

        let mut i = 0;
        let delim = delim.as_bytes(); // So that we can use [n].
        let mut delim_pos = 0;
        let mut next = [0u8];
        let mut in_delim = false;
        while i < max_len {
            self.recv(&mut next, 1)?;
            let value = next[0];

            // Unexpected character encountered while reading the delimiter. Bailing out.
            if in_delim && delim[delim_pos] != value {
                return Err((ChannelError::InvalidDelimiterChar));
            }

            if delim[delim_pos] == value {
                // We are advancing in the delimiter, eventually return if read in full.
                in_delim = true;
                delim_pos += 1;
                if delim_pos >= delim.len() {
                    // We found the last delimiter, return the slice.
                    let res = str::from_utf8(&data[0..i]);
                    if res.is_err() {
                        return Err(ChannelError::InvalidString);
                    }
                    return Ok(res.unwrap());
                }
            } else {
                // Not in the delimiter, add the current character to our buffer that we'll
                // slice when returning.
                data[i] = value;
                i += 1;
            }
        }

        // Not enough space in buffer, bailing out.
        Err((ChannelError::BufferFull))
    }
}

/// A simple channel implementation using a string as the source.
#[derive(Clone)]
pub struct StringChannel<'a> {
    pos: usize,
    data: &'a str,
}

impl<'a> StringChannel<'a> {
    pub fn new(data: &'a str) -> Self {
        StringChannel {
            pos: 0,
            data: data,
        }
    }
}

impl<'a> Channel for StringChannel<'a> {
    fn open(&mut self, _: &str, _: u16, _tls: bool) -> Result<(), ChannelError> {
        Ok(())
    }

    fn send(&mut self, _: &[u8], _: usize) -> Result<usize, ChannelError> {
        Ok(0)
    }

    fn recv(&mut self, data: &mut [u8], max_len: usize) -> Result<usize, ChannelError> {
        let bytes = self.data.as_bytes();
        let mut i = 0;
        loop {
            // We've read all we had to read.
            if i == max_len {
                break;
            }

            // We reached eof.
            if self.pos >= self.data.len() {
                return Err(ChannelError::EndOfStream);
            }

            // Everything is fine, copy this byte and advance.
            data[i] = bytes[self.pos];
            i += 1;
            self.pos += 1;
        }

        Ok(i)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_readline() {
        let mut buffer: [u8; 128] = [0; 128];
        let mut small_buffer: [u8; 8] = [0; 8];

        {
            let mut channel = StringChannel::new("This is a test string\r\n");
            let res = channel.read_string_until(&mut buffer, "\r\n").unwrap();
            assert_eq!(res, "This is a test string");
        }
        {
            let mut channel = StringChannel::new("This is a test string\r\nMore Data\r\n");
            {
                let res = channel.read_string_until(&mut buffer, "\r\n").unwrap();
                assert_eq!(res, "This is a test string");
            }
            {
                let res = channel.read_string_until(&mut buffer, "\r\n").unwrap();
                assert_eq!(res, "More Data");
            }
        }
        {
            let mut channel = StringChannel::new("This is a test string");
            let res = channel.read_string_until(&mut buffer, "\r\n");
            assert_eq!(res.err().unwrap(), ChannelError::EndOfStream);
        }
        {
            let mut channel = StringChannel::new("This is a test string\r\n");
            let res = channel.read_string_until(&mut small_buffer, "\r\n");
            assert_eq!(res.err().unwrap(), ChannelError::BufferFull);
        }
    }
}
