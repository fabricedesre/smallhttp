// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

#![no_std]

#![feature(collections)]

#[macro_use]
extern crate collections;

#[cfg(test)]
#[macro_use]
extern crate std;

/// A simple http library usable in embedded environments without std support.

use core::convert::From;

pub mod traits;
use traits::{Channel, ChannelError};

mod url;
// mod parser;

pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    fn as_str(&self) -> &str {
        let txt = match *self {
            HttpMethod::Get => "GET",
            HttpMethod::Head => "HEAD",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        };
        txt
    }
}

// TODO: complete this list.
pub enum HttpHeader {
    Host,
    ContentLength,
    ContentType,
    Etag,
}

impl HttpHeader {
    fn as_str(&self) -> &str {
        // We add the space after the header name to simplify serialization.
        let txt = match *self {
            HttpHeader::Host => "Host: ",
            HttpHeader::ContentLength => "Content-Length: ",
            HttpHeader::ContentType => "Content-Type: ",
            HttpHeader::Etag => "ETag: ",
        };
        txt
    }
}

static HTTP_VERSION: &'static str = " HTTP/1.1\r\n";
static LINE_END: &'static str = "\r\n";

#[derive(PartialEq, Debug, Clone)]
pub enum ClientState {
    Error,
    Created,
    HeadersOrBody,
    ReadResponse,
    Done,
}

#[derive(Debug)]
pub enum HttpError {
    BadState,
    BadUrl(url::UrlParsingError),
    ChannelError(ChannelError),
    UnsupportedScheme,
    UnknownError,
}

impl From<url::UrlParsingError> for HttpError {
    fn from(error: url::UrlParsingError) -> HttpError {
        HttpError::BadUrl(error)
    }
}

impl From<ChannelError> for HttpError {
    fn from(error: ChannelError) -> HttpError {
        HttpError::ChannelError(error)
    }
}

pub enum ResponseElement<'a> {
    Status(i16),
    Header(HttpHeader, &'a str),
    Body(&'a [u8]),
}

pub struct Client<'a, T> {
    channel: T,
    state: ClientState,
    method: HttpMethod,
    url: &'a str,
}

macro_rules! http_method {
    ($method:ident, $enumv:ident) => (
        pub fn $method(&'a mut self, url: &'a str) -> &mut Self {
            self.request(HttpMethod::$enumv, url)
        }
    )
}

impl<'a, T> Client<'a, T> {
    pub fn new(channel: T) -> Self
        where T: Channel
    {
        Client {
            channel: channel,
            state: ClientState::Error,
            method: HttpMethod::Get,
            url: "",
        }
    }

    pub fn open(&mut self) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::Created);

        self.state = ClientState::Error;

        // Get the host + port + secure state of the url and open the transport layer.
        let (scheme, host, port, path) = url::parse(self.url)?;
        if scheme != "http" && scheme != "https" {
            return Err(HttpError::UnsupportedScheme);
        }

        // Open the channel and send the initial part of the request.
        self.channel.open(host, port, scheme == "https")?;
        self.channel.send_str(self.method.as_str())?;
        self.channel.send_str(" ")?;
        self.channel.send_str(path)?;
        self.channel.send_str(HTTP_VERSION)?;
        // HTTP 1.1 only mandatory header is the Host one.
        self.channel.send_str(HttpHeader::Host.as_str())?;
        self.channel.send_str(host)?;
        self.channel.send_str(LINE_END)?;

        self.state = ClientState::HeadersOrBody;
        Ok(self)
    }

    pub fn headers(&mut self, headers: &[(HttpHeader, &str)]) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::HeadersOrBody);

        self.state = ClientState::Error;

        for header in headers {
            self.channel.send_str(header.0.as_str())?;
            self.channel.send_str(header.1)?;
            self.channel.send_str(LINE_END)?;
        }

        self.state = ClientState::HeadersOrBody;

        Ok(self)
    }

    pub fn header(&mut self, name: HttpHeader, value: &str) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        self.headers(&[(name, value)])
    }

    pub fn body(&mut self, body: &[u8]) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::HeadersOrBody);

        self.state = ClientState::Error;

        // Send the empty line after the headers, and then the body if it's not empty.
        self.channel.send_str(LINE_END)?;
        if body.len() != 0 {
            self.channel.send(body, body.len())?;
        }

        self.state = ClientState::ReadResponse;

        Ok(self)
    }

    // Reads a \r\n terminated line, up to the max length of the passed string.
    fn read_line(&mut self, buffer: &mut [u8]) -> Result<&str, HttpError>
        where T: Channel
    {
        let state = self.state.clone();
        self.state = ClientState::Error;
        let mut i: usize = 0;
        loop {
            if i == buffer.len() {
                break;
            }
        }
        Ok("")
    }

    pub fn response(&mut self, sink: fn(what: &ResponseElement)) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::ReadResponse);
        self.state = ClientState::Error;

        // Stack buffer for line based reads.
        let buffer = [0u8; 256];

        // Read the initial response line.

        self.state = ClientState::Done;
        Ok(self)
    }

    fn request(&'a mut self, method: HttpMethod, url: &'a str) -> &mut Self {
        self.url = url;
        self.method = method;
        self.state = ClientState::Created;
        self
    }

    http_method!(get, Get);
    http_method!(head, Head);
    http_method!(post, Post);
    http_method!(put, Put);
    http_method!(delete, Delete);
}

#[test]
fn test_get() {
    struct MockChannel {
        socket: i16,
    }

    impl MockChannel {
        fn new() -> Self {
            MockChannel { socket: -1 }
        }
    }

    impl Channel for MockChannel {
        fn open(&mut self, host: &str, port: i16, tls: bool) -> Result<(), ChannelError> {
            Ok(())
        }

        fn send(&self, data: &[u8], len: usize) -> Result<usize, ChannelError> {
            Ok(len)
        }

        fn recv(&self, data: &mut [u8], max_len: usize) -> Result<usize, ChannelError> {
            Ok(max_len)
        }

        fn new() -> Self {
            MockChannel { socket: 0 }
        }
    }

    let mut client = Client::new(MockChannel::new());
    client.get("http://localhost:8000/test.html").open().unwrap().body(&[]);
    assert_eq!(1, 1);
}
