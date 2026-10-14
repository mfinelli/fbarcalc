/* fbarcalc: find account maximum value
 * Copyright 2026 Mario Finelli
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Currency conversion using <https://freecurrencyapi.com>.

use serde_json::Value;
use std::error::Error;
use std::fmt;

const API_URL: &str = "https://api.freecurrencyapi.com/v1/latest";

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " https://github.com/mfinelli/fbarcalc"
);

#[derive(Debug)]
pub enum RateError {
    /// No API key is configured (and one is needed for this currency).
    MissingApiKey,
    /// The request couldn't be made or the response couldn't be read.
    Request(reqwest::Error),
    /// The API replied with an HTTP error.
    Status { code: u16, message: Option<String> },
    /// The API replied with something that isn't a rate for the currency.
    InvalidResponse,
    /// The API replied with a rate that can't be used (zero or negative).
    InvalidRate(f64),
}

impl fmt::Display for RateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RateError::MissingApiKey => write!(
                f,
                "no API key is configured, run `fbarcalc config` to set one"
            ),
            RateError::Request(e) => {
                write!(f, "unable to get the exchange rate: {}", e)
            }
            RateError::Status {
                code,
                message: Some(m),
            } => {
                write!(f, "the exchange rate API returned {}: {}", code, m)
            }
            RateError::Status {
                code,
                message: None,
            } => write!(f, "the exchange rate API returned {}", code),
            RateError::InvalidResponse => {
                write!(f, "the exchange rate API returned an unexpected reply")
            }
            RateError::InvalidRate(r) => {
                write!(
                    f,
                    "the exchange rate API returned an invalid rate: {}",
                    r
                )
            }
        }
    }
}

impl Error for RateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RateError::Request(e) => Some(e),
            _ => None,
        }
    }
}

/// Check that we are able to get the rate for the currency, without doing
/// any network requests. This lets us fail before the user has spent time
/// entering their transactions.
pub fn ensure_configured(
    api_key: Option<&str>,
    currency: &str,
) -> Result<(), RateError> {
    if currency == "USD" {
        return Ok(());
    }

    match api_key {
        Some(k) if !k.is_empty() => Ok(()),
        _ => Err(RateError::MissingApiKey),
    }
}

/// Get the amount of USD that one unit of the given currency is worth.
///
/// USD is always worth 1 so no API key is needed and no request is made.
pub fn usd_per_unit(
    api_key: Option<&str>,
    currency: &str,
) -> Result<f64, RateError> {
    usd_per_unit_from(API_URL, api_key, currency)
}

fn usd_per_unit_from(
    base_url: &str,
    api_key: Option<&str>,
    currency: &str,
) -> Result<f64, RateError> {
    ensure_configured(api_key, currency)?;

    if currency == "USD" {
        return Ok(1.0);
    }

    // ensure_configured guarantees that the key is present
    let body = fetch(base_url, api_key.unwrap_or_default(), currency)?;

    // the API gives us the value of one USD in the other currency but we
    // want the value of one unit of the currency in USD
    Ok(1.0 / parse_rate(&body, currency)?)
}

fn fetch(
    base_url: &str,
    api_key: &str,
    currency: &str,
) -> Result<String, RateError> {
    // reqwest includes the full url in its errors which would put the API key
    // (which is part of the query string) in our error messages
    let request_error = |e: reqwest::Error| RateError::Request(e.without_url());

    let client = reqwest::blocking::Client::builder()
        .gzip(true)
        .user_agent(USER_AGENT)
        .build()
        .map_err(request_error)?;

    let response = client
        .get(base_url)
        .query(&[
            ("apikey", api_key),
            ("base_currency", "USD"),
            ("currencies", currency),
        ])
        .send()
        .map_err(request_error)?;

    let status = response.status();
    let body = response.text().map_err(request_error)?;

    if status.is_success() {
        Ok(body)
    } else {
        Err(RateError::Status {
            code: status.as_u16(),
            message: api_message(&body),
        })
    }
}

/// Get the rate for the currency out of the API's response body.
fn parse_rate(body: &str, currency: &str) -> Result<f64, RateError> {
    let v: Value =
        serde_json::from_str(body).map_err(|_| RateError::InvalidResponse)?;

    let rate = v["data"][currency]
        .as_f64()
        .ok_or(RateError::InvalidResponse)?;

    // the rate is used as a divisor so it has to be positive and finite
    if rate.is_finite() && rate > 0.0 {
        Ok(rate)
    } else {
        Err(RateError::InvalidRate(rate))
    }
}

/// Get the human-readable message out of an API error body, if there is one.
fn api_message(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v["message"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    // a url that nothing is listening on: any attempt to use it is an error
    const UNREACHABLE: &str = "http://127.0.0.1:1/v1/latest";

    #[test]
    fn parse_rate_reads_the_requested_currency() {
        let body = r#"{"data":{"EUR":0.92}}"#;
        assert_eq!(parse_rate(body, "EUR").unwrap(), 0.92);
    }

    #[test]
    fn parse_rate_accepts_whole_numbers() {
        let body = r#"{"data":{"JPY":150}}"#;
        assert_eq!(parse_rate(body, "JPY").unwrap(), 150.0);
    }

    #[test]
    fn parse_rate_ignores_other_currencies() {
        let body = r#"{"data":{"EUR":0.92,"GBP":0.79}}"#;
        assert_eq!(parse_rate(body, "GBP").unwrap(), 0.79);
    }

    #[test]
    fn parse_rate_rejects_unexpected_replies() {
        for body in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"data":{}}"#,
            r#"{"data":[]}"#,
            r#"{"data":null}"#,
            r#"{"data":{"GBP":0.79}}"#,
            r#"{"data":{"EUR":null}}"#,
            r#"{"data":{"EUR":"0.92"}}"#,
            r#"{"message":"Validation error"}"#,
        ] {
            assert!(
                matches!(
                    parse_rate(body, "EUR"),
                    Err(RateError::InvalidResponse)
                ),
                "body {:?}",
                body
            );
        }
    }

    #[test]
    fn parse_rate_rejects_rates_that_cant_be_used_as_a_divisor() {
        for (body, bad) in [
            (r#"{"data":{"EUR":0}}"#, 0.0),
            (r#"{"data":{"EUR":0.0}}"#, 0.0),
            (r#"{"data":{"EUR":-0.92}}"#, -0.92),
        ] {
            match parse_rate(body, "EUR") {
                Err(RateError::InvalidRate(r)) => assert_eq!(r, bad),
                other => panic!("body {:?}: {:?}", body, other),
            }
        }
    }

    #[test]
    fn api_message_reads_the_message_field() {
        let body = r#"{"message":"Invalid authentication credentials"}"#;
        assert_eq!(
            api_message(body).as_deref(),
            Some("Invalid authentication credentials")
        );
    }

    #[test]
    fn api_message_is_none_without_a_usable_message() {
        assert_eq!(api_message(""), None);
        assert_eq!(api_message("<html>Bad Gateway</html>"), None);
        assert_eq!(api_message("{}"), None);
        assert_eq!(api_message(r#"{"message":5}"#), None);
    }

    #[test]
    fn ensure_configured_needs_a_key_for_anything_but_usd() {
        assert!(ensure_configured(None, "USD").is_ok());
        assert!(ensure_configured(Some(""), "USD").is_ok());
        assert!(ensure_configured(Some("key"), "USD").is_ok());
        assert!(ensure_configured(Some("key"), "EUR").is_ok());
        assert!(matches!(
            ensure_configured(None, "EUR"),
            Err(RateError::MissingApiKey)
        ));
        assert!(matches!(
            ensure_configured(Some(""), "EUR"),
            Err(RateError::MissingApiKey)
        ));
    }

    #[test]
    fn usd_is_worth_one_usd_without_making_a_request() {
        assert_eq!(usd_per_unit_from(UNREACHABLE, None, "USD").unwrap(), 1.0);
        assert_eq!(
            usd_per_unit_from(UNREACHABLE, Some("k"), "USD").unwrap(),
            1.0
        );
    }

    #[test]
    fn a_missing_key_fails_without_making_a_request() {
        assert!(matches!(
            usd_per_unit_from(UNREACHABLE, None, "EUR"),
            Err(RateError::MissingApiKey)
        ));
        assert!(matches!(
            usd_per_unit_from(UNREACHABLE, Some(""), "EUR"),
            Err(RateError::MissingApiKey)
        ));
    }

    #[test]
    fn the_missing_key_message_says_how_to_fix_it() {
        let msg = RateError::MissingApiKey.to_string();
        assert!(msg.contains("fbarcalc config"), "{}", msg);
    }

    #[test]
    fn the_rate_is_inverted_to_usd_per_unit() {
        // the API says 1 USD = 0.5 EUR so 1 EUR = 2 USD
        let (url, server) =
            serve_once("200 OK", &[], br#"{"data":{"EUR":0.5}}"#.to_vec());
        assert_eq!(usd_per_unit_from(&url, Some("k"), "EUR").unwrap(), 2.0);
        server.join().unwrap();
    }

    #[test]
    fn the_request_is_what_the_api_expects() {
        let (url, server) =
            serve_once("200 OK", &[], br#"{"data":{"JPY":150}}"#.to_vec());
        usd_per_unit_from(&url, Some("secret-key"), "JPY").unwrap();

        let request = server.join().unwrap().to_lowercase();
        let request_line = request.lines().next().unwrap();
        assert!(request_line.starts_with("get /v1/latest?"), "{}", request);
        assert!(request_line.contains("apikey=secret-key"), "{}", request);
        assert!(request_line.contains("base_currency=usd"), "{}", request);
        assert!(request_line.contains("currencies=jpy"), "{}", request);
        assert!(
            request.contains("user-agent: fbarcalc/")
                && request.contains("github.com/mfinelli/fbarcalc"),
            "{}",
            request
        );
        assert!(request.contains("accept-encoding: gzip"), "{}", request);
    }

    #[test]
    fn gzipped_responses_are_decoded() {
        let body = gzip(br#"{"data":{"EUR":0.5}}"#);
        let (url, server) =
            serve_once("200 OK", &[("Content-Encoding", "gzip")], body);
        assert_eq!(usd_per_unit_from(&url, Some("k"), "EUR").unwrap(), 2.0);
        server.join().unwrap();
    }

    #[test]
    fn http_errors_include_the_api_message() {
        let (url, server) = serve_once(
            "401 Unauthorized",
            &[],
            br#"{"message":"Invalid authentication credentials"}"#.to_vec(),
        );
        let err = usd_per_unit_from(&url, Some("bad"), "EUR").unwrap_err();
        server.join().unwrap();

        match &err {
            RateError::Status { code, message } => {
                assert_eq!(*code, 401);
                assert_eq!(
                    message.as_deref(),
                    Some("Invalid authentication credentials")
                );
            }
            other => panic!("{:?}", other),
        }
        assert_eq!(
            err.to_string(),
            "the exchange rate API returned 401: \
             Invalid authentication credentials"
        );
    }

    #[test]
    fn http_errors_without_a_message_still_report_the_status() {
        let (url, server) = serve_once(
            "502 Bad Gateway",
            &[],
            b"<html>Bad Gateway</html>".to_vec(),
        );
        let err = usd_per_unit_from(&url, Some("k"), "EUR").unwrap_err();
        server.join().unwrap();

        assert!(matches!(
            err,
            RateError::Status {
                code: 502,
                message: None
            }
        ));
        assert_eq!(err.to_string(), "the exchange rate API returned 502");
    }

    #[test]
    fn an_unexpected_success_body_is_an_error() {
        let (url, server) =
            serve_once("200 OK", &[], b"definitely not json".to_vec());
        assert!(matches!(
            usd_per_unit_from(&url, Some("k"), "EUR"),
            Err(RateError::InvalidResponse)
        ));
        server.join().unwrap();
    }

    #[test]
    fn a_zero_rate_is_an_error_instead_of_infinity() {
        let (url, server) =
            serve_once("200 OK", &[], br#"{"data":{"EUR":0}}"#.to_vec());
        assert!(matches!(
            usd_per_unit_from(&url, Some("k"), "EUR"),
            Err(RateError::InvalidRate(_))
        ));
        server.join().unwrap();
    }

    #[test]
    fn connection_errors_do_not_leak_the_api_key() {
        let err = usd_per_unit_from(UNREACHABLE, Some("secret-key"), "EUR")
            .unwrap_err();
        assert!(matches!(err, RateError::Request(_)));

        let shown = err.to_string();
        let debug = format!("{:?}", err);
        let source = err.source().map(|s| s.to_string()).unwrap_or_default();
        for text in [shown, debug, source] {
            assert!(!text.contains("secret-key"), "{}", text);
            assert!(!text.contains("apikey"), "{}", text);
        }
    }

    /// Start a one-shot HTTP server on a local port. It answers the first
    /// request with the given response and then returns the raw text of that
    /// request (through the join handle) so that it can be inspected.
    fn serve_once(
        status: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url =
            format!("http://{}/v1/latest", listener.local_addr().unwrap());

        let mut head = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            status,
            body.len()
        );
        for (name, value) in headers {
            head.push_str(&format!("{}: {}\r\n", name, value));
        }
        head.push_str("\r\n");

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
            }

            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();

            String::from_utf8_lossy(&request).into_owned()
        });

        (url, handle)
    }

    /// A minimal gzip file: a single uncompressed ("stored") deflate block.
    fn gzip(data: &[u8]) -> Vec<u8> {
        assert!(data.len() <= u16::MAX as usize);
        let len = data.len() as u16;

        let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
        out.push(1); // final block, stored
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(data).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
