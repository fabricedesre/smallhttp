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

use collections::{String, Vec};
use collections::borrow::ToOwned;
use core::convert::From;
use core::ops::Fn;
use core::str::FromStr;
use core::str;

pub mod traits;
use traits::{Channel, ChannelError, StringChannel};

pub mod url;

pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    fn as_str(&self) -> &str {
        match *self {
            HttpMethod::Get => "GET",
            HttpMethod::Head => "HEAD",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
}

// TODO: complete this list.
#[derive(Clone, Debug, PartialEq)]
pub enum HttpHeader {
    Connection,
    ContentLength,
    ContentType,
    Date,
    Etag,
    Host,
    LastModified,
    Server,
    Other(String),
}

impl From<String> for HttpHeader {
    fn from(item: String) -> HttpHeader {
        if item == "Connection:" {
            HttpHeader::Connection
        } else if item == "Content-Length:" {
            HttpHeader::ContentLength
        } else if item == "Content-Type:" {
            HttpHeader::ContentType
        } else if item == "Date:" {
            HttpHeader::Date
        } else if item == "ETag:" {
            HttpHeader::Etag
        } else if item == "Host:" {
            HttpHeader::Host
        } else if item == "Last-Modified:" {
            HttpHeader::LastModified
        } else if item == "Server:" {
            HttpHeader::Server
        } else {
            HttpHeader::Other(String::from(item))
        }
    }
}

impl HttpHeader {
    fn as_string(&self) -> String {
        // We add the space after the header name to simplify serialization.
        match *self {
            HttpHeader::Connection => "Connection: ".to_owned(),
            HttpHeader::ContentLength => "Content-Length: ".to_owned(),
            HttpHeader::ContentType => "Content-Type: ".to_owned(),
            HttpHeader::Date => "Date: ".to_owned(),
            HttpHeader::Etag => "ETag: ".to_owned(),
            HttpHeader::Host => "Host: ".to_owned(),
            HttpHeader::LastModified => "LastModified: ".to_owned(),
            HttpHeader::Server => "Server: ".to_owned(),
            HttpHeader::Other(ref name) => format!("{} ", name),
        }
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

#[derive(Debug, PartialEq)]
pub enum HttpError {
    BadState,
    BadUrl(url::UrlParsingError),
    ChannelError(ChannelError),
    UnsupportedScheme,
    UnknownError,
    InvalidVersion,
    InvalidStatusCode,
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

impl From<HttpError> for () {
    fn from(_: HttpError) -> () {
        ()
    }
}

pub struct Response<'a, T: 'a> {
    pub status_code: u16,
    pub status: String,
    pub headers: Vec<(HttpHeader, String)>,
    pub body: &'a mut T,
}

pub struct Client<'a, T> {
    channel: T,
    state: ClientState,
    method: HttpMethod,
    url: &'a str,
    headers_flushed: bool,
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
            headers_flushed: false,
        }
    }

    pub fn open(&mut self) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::Created);

        self.state = ClientState::Error;

        // Get the host + port + secure state of the url and open the transport layer.
        let (scheme, host, port, path) = url::parse_url(self.url)?;
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
        self.channel.send_str(&HttpHeader::Host.as_string())?;
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
            self.channel.send_str(&header.0.as_string())?;
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

    fn _send(&mut self, body: &[u8], final_state: ClientState) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        assert_eq!(self.state, ClientState::HeadersOrBody);

        self.state = ClientState::Error;

        // Send the empty line after the headers, and then the body if it's not empty.
        if !self.headers_flushed {
            self.headers_flushed = true;
            self.channel.send_str(LINE_END)?;
        }

        if body.len() != 0 {
            self.channel.send(body, body.len())?;
        }

        self.state = final_state;

        Ok(self)
    }

    // Sends a part of the body. Can be called multiple times before a send()
    pub fn body(&mut self, body: &[u8]) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        self._send(body, ClientState::HeadersOrBody)
    }

    // Last or single send of a sequence.
    pub fn send(&mut self, body: &[u8]) -> Result<&mut Self, HttpError>
        where T: Channel
    {
        self._send(body, ClientState::ReadResponse)
    }

    pub fn response<F>(&mut self, filter: F) -> Result<Response<T>, HttpError>
        where T: Channel + Clone,
              F: Fn(HttpHeader) -> bool
    {
        // Some methods don't need a body, so if we are in HeadersOrBody state, just
        // trigger an empty send().
        if self.state == ClientState::HeadersOrBody {
            self.send(&[])?;
        }

        assert_eq!(self.state, ClientState::ReadResponse);
        self.state = ClientState::Error;

        let mut buffer = [0u8; 256];
        let buff_size = buffer.len();

        let status_line = String::from(self.channel.read_string_until(&mut buffer, "\r\n")?);

        let mut channel = StringChannel::new(&status_line);
        let http_version = String::from(channel.read_string_until(&mut buffer, " ")?);
        // Accept both HTTP 1.0 and 1.1.
        if http_version != "HTTP/1.0" && http_version != "HTTP/1.1" {
            return Err(HttpError::InvalidVersion);
        }
        let status_code = u16::from_str(channel.read_string_until(&mut buffer, " ")?)
            .map_err(|_| HttpError::InvalidStatusCode)?;

        // The status is the remainder of the line.
        let size = channel.read_to_end(&mut buffer, buff_size)?;
        let status = String::from(str::from_utf8(&buffer[0..size]).unwrap());

        // Read headers.
        let mut headers = Vec::new();
        loop {
            let header_line = String::from(self.channel.read_string_until(&mut buffer, "\r\n")?);
            if header_line.is_empty() {
                break;
            }

            let mut channel = StringChannel::new(&header_line);
            let header_name = String::from(channel.read_string_until(&mut buffer, " ")?);

            // Check if we are interested in this header before reading the value.
            let header_name = HttpHeader::from(header_name);
            if filter(header_name.clone()) {
                // The status is the remainder of the line.
                let size = channel.read_to_end(&mut buffer, buff_size)?;
                let header_value = String::from(str::from_utf8(&buffer[0..size]).unwrap());
                headers.push((header_name, header_value));
            }
        }

        self.state = ClientState::Done;
        Ok(Response {
            status_code: status_code,
            status: status,
            headers: headers,
            body: &mut self.channel,
        })
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
    let http_channel = StringChannel::new("HTTP/1.1 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.get("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .send(&[])
        .unwrap()
        .response(|_| true)
        .unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.status, "OK");
    assert_eq!(response.headers.len(), 2);
    assert_eq!(response.headers[0],
               (HttpHeader::ContentType, String::from("text/html; charset=UTF-8")));
    assert_eq!(response.headers[1],
               (HttpHeader::ContentLength, String::from("138")));
}

#[test]
fn test_get_no_send() {
    let http_channel = StringChannel::new("HTTP/1.1 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.get("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .response(|_| true)
        .unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.status, "OK");
    assert_eq!(response.headers.len(), 2);
    assert_eq!(response.headers[0],
               (HttpHeader::ContentType, String::from("text/html; charset=UTF-8")));
    assert_eq!(response.headers[1],
               (HttpHeader::ContentLength, String::from("138")));
}

#[test]
fn test_get_1_0() {
    let http_channel = StringChannel::new("HTTP/1.0 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.get("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .send(&[])
        .unwrap()
        .response(|_| true)
        .unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.status, "OK");
    assert_eq!(response.headers.len(), 2);
    assert_eq!(response.headers[0],
               (HttpHeader::ContentType, String::from("text/html; charset=UTF-8")));
    assert_eq!(response.headers[1],
               (HttpHeader::ContentLength, String::from("138")));
}

#[test]
fn test_get_1_2() {
    let http_channel = StringChannel::new("HTTP/1.2 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.get("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .send(&[])
        .unwrap()
        .response(|_| true);
    assert_eq!(response.err().unwrap(), HttpError::InvalidVersion);
}

#[test]
fn test_post() {
    let http_channel = StringChannel::new("HTTP/1.1 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.post("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .send(&[])
        .unwrap()
        .response(|header_name| header_name == HttpHeader::ContentType)
        .unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.status, "OK");
    assert_eq!(response.headers.len(), 1);
    assert_eq!(response.headers[0],
               (HttpHeader::ContentType, String::from("text/html; charset=UTF-8")));
}

#[test]
fn test_body() {
    let http_channel = StringChannel::new("HTTP/1.1 200 OK\r\nContent-Type: text/html; \
                                           charset=UTF-8\r\nContent-Length: \
                                           138\r\n\r\n<html><head><title>An Example \
                                           Page</title></head><body>Hello World, this is a very \
                                           simple HTML document.</body></html>");
    let mut client = Client::new(http_channel);
    let response = client.get("http://localhost:8000/test.html")
        .open()
        .unwrap()
        .send(&[])
        .unwrap()
        .response(|_| true)
        .unwrap();
    let mut buffer = [0u8; 256];
    let s = response.body.read_string_to_end(&mut buffer).unwrap();
    assert_eq!(s,
               "<html><head><title>An Example Page</title></head><body>Hello World, this is a \
                very simple HTML document.</body></html>");
}
