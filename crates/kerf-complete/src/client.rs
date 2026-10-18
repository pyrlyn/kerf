// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One fill-in-the-middle request, blocking. Runs on the completer's worker
//! thread, never on the event loop.

use std::fmt;
use std::io::ErrorKind;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::context::Context;
use crate::error::Error;

/// runa's own upper bound for `n_predict`; asking for more is refused there.
pub const MAX_N_PREDICT: u32 = 4096;

/// How much of an error body ends up in [`Error::Status`]: enough for runa's
/// one-line reasons, not enough to flood a status bar.
const MAX_STATUS_MESSAGE_CHARS: usize = 200;

/// Where to send requests and how far to trust the replies.
#[derive(Clone)]
pub struct Config {
    /// Full URL of the endpoint, e.g. `http://127.0.0.1:8080/v1/infill`.
    pub endpoint: String,
    /// Sent as `Authorization: Bearer …` when set (runa's `--api-key`).
    pub api_key: Option<String>,
    /// Model id for a server that holds several; the server's default if unset.
    pub model: Option<String>,
    /// Whole-request limit, connect to last body byte. Inline completion is
    /// only useful while the user waits, so this is short.
    pub timeout: Duration,
    /// Most reply bytes read before the request is refused.
    pub max_reply_bytes: u64,
    /// Longest completion text accepted, in bytes.
    pub max_completion_bytes: usize,
    /// Most bytes of text before the cursor sent.
    pub max_prefix_bytes: usize,
    /// Most bytes of text after the cursor sent.
    pub max_suffix_bytes: usize,
    /// Most tokens the model may generate.
    pub n_predict: u32,
}

impl Config {
    /// Defaults sized for inline suggestions of a few lines.
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: None,
            model: None,
            timeout: Duration::from_secs(5),
            max_reply_bytes: 64 * 1024,
            max_completion_bytes: 16 * 1024,
            max_prefix_bytes: 16 * 1024,
            max_suffix_bytes: 4 * 1024,
            n_predict: 128,
        }
    }

    fn check(&self) -> Result<(), Error> {
        if !(self.endpoint.starts_with("http://") || self.endpoint.starts_with("https://")) {
            return Err(Error::Config("endpoint must be an http:// or https:// URL"));
        }
        if self.timeout.is_zero() {
            return Err(Error::Config("timeout must be above zero"));
        }
        if self.max_reply_bytes == 0 || self.max_completion_bytes == 0 {
            return Err(Error::Config(
                "reply and completion caps must be above zero",
            ));
        }
        if self.max_prefix_bytes == 0 && self.max_suffix_bytes == 0 {
            return Err(Error::Config("prefix and suffix caps cannot both be zero"));
        }
        if !(1..=MAX_N_PREDICT).contains(&self.n_predict) {
            return Err(Error::Config("n_predict must be between 1 and 4096"));
        }
        Ok(())
    }
}

// By hand so the API key never reaches a log line.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .field("max_reply_bytes", &self.max_reply_bytes)
            .field("max_completion_bytes", &self.max_completion_bytes)
            .field("max_prefix_bytes", &self.max_prefix_bytes)
            .field("max_suffix_bytes", &self.max_suffix_bytes)
            .field("n_predict", &self.n_predict)
            .finish()
    }
}

/// The part of the `/v1/infill` reply Kerf reads; other fields are ignored.
#[derive(Deserialize)]
struct Reply {
    content: String,
}

/// A configured fill-in-the-middle client.
#[derive(Debug)]
pub struct Client {
    agent: ureq::Agent,
    config: Config,
}

impl Client {
    /// A client for `config`, refused if a field is out of range.
    pub fn new(config: Config) -> Result<Self, Error> {
        config.check()?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(config.timeout))
            // Error statuses carry runa's reason; read them instead of
            // getting an opaque error.
            .http_status_as_error(false)
            // The endpoint is configured, not discovered: a redirect would
            // send the user's code somewhere nobody chose.
            .max_redirects(0)
            // Same reason: an `HTTP_PROXY` set for other tools must not see
            // the source text of every keystroke.
            .proxy(None)
            .build()
            .into();
        Ok(Self { agent, config })
    }

    /// The settings this client was built with.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Send `context` and return the middle the model suggests.
    pub fn complete(&self, context: &Context) -> Result<String, Error> {
        let body = self.body(context);
        let mut request = self
            .agent
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        let mut response = request.send(body).map_err(|e| self.map_err(e))?;
        let status = response.status();
        let bytes = response
            .body_mut()
            .with_config()
            .limit(self.config.max_reply_bytes)
            .read_to_vec()
            .map_err(|e| self.map_err(e))?;
        if !status.is_success() {
            let message = String::from_utf8_lossy(&bytes)
                .chars()
                .take(MAX_STATUS_MESSAGE_CHARS)
                .collect();
            return Err(Error::Status {
                status: status.as_u16(),
                message,
            });
        }
        let reply: Reply =
            serde_json::from_slice(&bytes).map_err(|e| Error::BadReply(e.to_string()))?;
        if reply.content.len() > self.config.max_completion_bytes {
            return Err(Error::TooLong {
                len: reply.content.len(),
                limit: self.config.max_completion_bytes,
            });
        }
        Ok(reply.content)
    }

    /// llama-server's `/infill` fields, which runa's `/v1/infill` takes.
    fn body(&self, context: &Context) -> String {
        let mut body = Map::new();
        body.insert("input_prefix".into(), context.prefix.clone().into());
        body.insert("input_suffix".into(), context.suffix.clone().into());
        body.insert("n_predict".into(), self.config.n_predict.into());
        if let Some(model) = &self.config.model {
            body.insert("model".into(), model.clone().into());
        }
        Value::Object(body).to_string()
    }

    fn map_err(&self, err: ureq::Error) -> Error {
        match err {
            ureq::Error::Timeout(_) => Error::Timeout,
            ureq::Error::Io(e) if e.kind() == ErrorKind::TimedOut => Error::Timeout,
            ureq::Error::BodyExceedsLimit(_) => Error::ReplyTooLarge {
                limit: self.config.max_reply_bytes,
            },
            other => Error::Transport(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_configs_are_refused() {
        let ok = Config::new("http://127.0.0.1:8080/v1/infill");
        assert!(Client::new(ok.clone()).is_ok());
        let cases: [fn(&mut Config); 6] = [
            |c| c.endpoint = "file:///etc/passwd".into(),
            |c| c.timeout = Duration::ZERO,
            |c| c.max_reply_bytes = 0,
            |c| (c.max_prefix_bytes, c.max_suffix_bytes) = (0, 0),
            |c| c.n_predict = 0,
            |c| c.n_predict = MAX_N_PREDICT + 1,
        ];
        for break_it in cases {
            let mut c = ok.clone();
            break_it(&mut c);
            assert!(matches!(Client::new(c), Err(Error::Config(_))));
        }
    }

    #[test]
    fn debug_output_hides_the_api_key() {
        let mut c = Config::new("http://127.0.0.1:8080/v1/infill");
        c.api_key = Some("sk-secret".into());
        let shown = format!("{c:?}");
        assert!(!shown.contains("sk-secret"), "{shown}");
        assert!(shown.contains("<redacted>"));
    }

    #[test]
    fn the_body_uses_llama_server_field_names() {
        let mut c = Config::new("http://127.0.0.1:8080/v1/infill");
        c.model = Some("coder".into());
        c.n_predict = 32;
        let client = Client::new(c).unwrap();
        let body: Value = serde_json::from_str(&client.body(&Context {
            prefix: "a".into(),
            suffix: "b".into(),
        }))
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "input_prefix": "a",
                "input_suffix": "b",
                "n_predict": 32,
                "model": "coder",
            })
        );
    }
}
