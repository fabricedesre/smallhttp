// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

// A simple url parser (No idna support).

use core::convert::From;
use core::num;
use core::str;
use core::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum UrlParsingError {
    Utf8Error(str::Utf8Error),
    ParseIntError(num::ParseIntError),
    DelimiterNotFound,
    UnexpectedError,
}

impl From<str::Utf8Error> for UrlParsingError {
    fn from(err: str::Utf8Error) -> Self {
        UrlParsingError::Utf8Error(err)
    }
}

impl From<num::ParseIntError> for UrlParsingError {
    fn from(err: num::ParseIntError) -> Self {
        UrlParsingError::ParseIntError(err)
    }
}

fn until_and_consume(input: &[u8], delim: u8) -> Result<(&[u8], &[u8]), UrlParsingError> {
    for (i, item) in input.iter().enumerate() {
        if *item == delim {
            return Ok((&input[i + 1..], &input[0..i]));
        }
    }
    // Delimiter not found...
    Err(UrlParsingError::DelimiterNotFound)
}

fn until(input: &[u8], delim: u8) -> Result<(&[u8], &[u8]), UrlParsingError> {
    for (i, item) in input.iter().enumerate() {
        if *item == delim {
            return Ok((&input[i..], &input[0..i]));
        }
    }
    // Delimiter not found...
    Err(UrlParsingError::DelimiterNotFound)
}

fn first_pos_of(input: &[u8], delim: u8) -> Option<usize> {
    for (i, item) in input.iter().enumerate() {
        if *item == delim {
            return Some(i);
        }
    }
    None
}

// Returns (scheme, host, port, path)
pub fn parse_url(url: &str) -> Result<(&str, &str, u16, &str), UrlParsingError> {
    let buffer = url.as_bytes();

    // Get the scheme
    let mut res = until_and_consume(buffer, b':')?;
    let scheme = str::from_utf8(res.1)?;

    res = until_and_consume(res.0, b'/')?;
    res = until_and_consume(res.0, b'/')?;

    // Check if we have a `:` and/or `/` and in which order, to figure out if there is a port
    // number and a non default path.

    let c_pos = first_pos_of(res.0, b':');
    let s_pos = first_pos_of(res.0, b'/');

    let host;
    let mut path = "/";
    let mut port: u16 = match scheme {
        "http" => 80,
        "https" => 443,
        _ => 0,
    };

    if c_pos.is_some() && s_pos.is_some() {
        if c_pos.unwrap() < s_pos.unwrap() {
            // We have a : before /, split the host:port fragment.
            res = until_and_consume(res.0, b':')?;
            host = str::from_utf8(res.1)?;
            res = until(res.0, b'/')?;
            let port_string = str::from_utf8(res.1)?;
            port = u16::from_str(port_string)?;
        } else {
            // The : is after /, hence not a port delimiter.
            res = until(res.0, b'/')?;
            host = str::from_utf8(res.1)?;
        }

        // The remaining part of the url is the path.
        // We remove the # part if any.
        if first_pos_of(res.0, b'#').is_some() {
            res = until_and_consume(res.0, b'#')?;
            path = str::from_utf8(res.1)?;
        } else {
            path = str::from_utf8(res.0)?;
        }
    } else if !s_pos.is_some() {
        // No / found, just use the remaining as the host:port
        if c_pos.is_some() {
            res = until_and_consume(res.0, b':')?;
            host = str::from_utf8(res.1)?;
            let port_string = str::from_utf8(res.0)?;
            port = u16::from_str(port_string)?;
        } else {
            host = str::from_utf8(res.0)?;
        }
    } else {
        // There is a /, split the host and path.
        res = until(res.0, b'/')?;
        host = str::from_utf8(res.1)?;

        // The remaining part of the url is the path.
        // We remove the # part if any.
        if first_pos_of(res.0, b'#').is_some() {
            res = until_and_consume(res.0, b'#')?;
            path = str::from_utf8(res.1)?;
        } else {
            path = str::from_utf8(res.0)?;
        }
    }


    Ok((scheme, host, port, path))
}

#[test]
fn url_test() {
    let url = parse_url("http://localhost").unwrap();
    assert_eq!(url, ("http", "localhost", 80, "/"));

    let url = parse_url("http://example.com/").unwrap();
    assert_eq!(url, ("http", "example.com", 80, "/"));

    let url = parse_url("http://example.com/path/to/file.html").unwrap();
    assert_eq!(url, ("http", "example.com", 80, "/path/to/file.html"));

    let url = parse_url("https://localhost").unwrap();
    assert_eq!(url, ("https", "localhost", 443, "/"));

    let url = parse_url("http://localhost:8080").unwrap();
    assert_eq!(url, ("http", "localhost", 8080, "/"));

    let url = parse_url("http://localhost:8080/").unwrap();
    assert_eq!(url, ("http", "localhost", 8080, "/"));

    let url = parse_url("http://localhost:8080/index.html").unwrap();
    assert_eq!(url, ("http", "localhost", 8080, "/index.html"));

    let url = parse_url("http://localhost:8080/index.html#hash").unwrap();
    assert_eq!(url, ("http", "localhost", 8080, "/index.html"));

    let url = parse_url("http://localhost:8000/path/to/index.html?foo=bar").unwrap();
    assert_eq!(url,
               ("http", "localhost", 8000, "/path/to/index.html?foo=bar"));

    let url = parse_url("http://api.bewrosnes.org/v1.0/Datastreams").unwrap();
    assert_eq!(url, ("http", "api.bewrosnes.org", 80, "/v1.0/Datastreams"));
}
