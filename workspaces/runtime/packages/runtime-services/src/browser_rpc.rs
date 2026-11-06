//! Bounded socket RPC for the host browser bridge.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use studio_json::{Map, Value};
use thiserror::Error;
const FRAME_LIMIT_BYTES: usize = 2 * 1024 * 1024;
const RPC_TIMEOUT_SECONDS: u64 = 3_600;
const CONNECT_TIMEOUT_SECONDS: u64 = 15;
const BUFFER_READ_BYTES: usize = 65_536;
const REQUEST_ID_MAX_CHARS: usize = 128;
#[derive(Debug, Error)]
pub enum BrowserRpcError {
    #[error("invalid browser bridge request")]
    InvalidRequest,
    #[error("could not create browser bridge request identity")]
    Identity,
    #[error("browser bridge transport failed: {message}")]
    Transport { message: String, uncertain: bool },
    #[error("browser bridge protocol failed: {message}")]
    Protocol {
        message: &'static str,
        uncertain: bool,
    },
    #[error("browser bridge rejected the request")]
    Rejected {
        code: Option<String>,
        detail: Option<String>,
        uncertain: bool,
    },
    #[error("browser bridge frame callback failed")]
    FrameCallback { uncertain: bool },
}

impl BrowserRpcError {
    /// Whether the browser bridge may have completed the request without the caller receiving its result.
    pub fn uncertain(&self) -> bool {
        match self {
            Self::Transport { uncertain, .. }
            | Self::Protocol { uncertain, .. }
            | Self::Rejected { uncertain, .. } => *uncertain,
            Self::InvalidRequest | Self::Identity => false,
            Self::FrameCallback { uncertain } => *uncertain,
        }
    }

    /// The browser RPC error code, if it returned one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Rejected { code, .. } => code.as_deref(),
            Self::InvalidRequest
            | Self::Identity
            | Self::Transport { .. }
            | Self::Protocol { .. }
            | Self::FrameCallback { .. } => None,
        }
    }

    /// The guest's error message, if it returned one.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Rejected { detail, .. } => detail.as_deref(),
            Self::InvalidRequest
            | Self::Identity
            | Self::Transport { .. }
            | Self::Protocol { .. }
            | Self::FrameCallback { .. } => None,
        }
    }
}

/// One fakeable synchronous boundary for JSON-RPC calls to the browser bridge.
pub trait BrowserRpc: Send + Sync {
    /// Send one request and return its final `result` frame.
    fn call(
        &self,
        method: &str,
        params: &Value,
        request_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, BrowserRpcError> {
        let mut ignore_frame = |_| Ok(());
        self.stream(method, params, request_id, timeout, &mut ignore_frame)
    }

    /// Send one request, deliver every response frame to `on_frame`, and return the final result.
    fn stream(
        &self,
        method: &str,
        params: &Value,
        request_id: Option<&str>,
        timeout: Duration,
        on_frame: &mut dyn FnMut(Value) -> Result<(), BrowserRpcError>,
    ) -> Result<Value, BrowserRpcError>;
}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct UnixSocketBrowserRpc {
    socket_path: PathBuf,
}

#[cfg(unix)]
impl UnixSocketBrowserRpc {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }
}

#[cfg(unix)]
impl BrowserRpc for UnixSocketBrowserRpc {
    fn stream(
        &self,
        method: &str,
        params: &Value,
        request_id: Option<&str>,
        timeout: Duration,
        on_frame: &mut dyn FnMut(Value) -> Result<(), BrowserRpcError>,
    ) -> Result<Value, BrowserRpcError> {
        stream_socket(
            &self.socket_path,
            method,
            params,
            request_id,
            timeout,
            on_frame,
        )
    }
}

#[cfg(unix)]
fn stream_socket(
    socket_path: &Path,
    method: &str,
    params: &Value,
    request_id: Option<&str>,
    timeout: Duration,
    on_frame: &mut dyn FnMut(Value) -> Result<(), BrowserRpcError>,
) -> Result<Value, BrowserRpcError> {
    use socket2::{Domain, SockAddr, Socket, Type};
    use std::os::unix::net::UnixStream;

    let identity = request_identity(request_id)?;
    validate_request(method, &identity, timeout)?;
    let mut request = Map::new();
    request.insert("id".to_owned(), Value::String(identity.clone()));
    request.insert("method".to_owned(), Value::String(method.to_owned()));
    request.insert(
        "params".to_owned(),
        if params.is_null() {
            Value::Object(Map::new())
        } else {
            params.clone()
        },
    );
    let mut frame = studio_json::to_vec(&Value::Object(request))
        .map_err(|_| BrowserRpcError::InvalidRequest)?;
    frame.push(b'\n');
    if frame.len() > FRAME_LIMIT_BYTES {
        return Err(BrowserRpcError::Protocol {
            message: "request exceeds the frame limit",
            uncertain: false,
        });
    }

    let deadline = Instant::now() + timeout;
    let address = SockAddr::unix(socket_path).map_err(|error| BrowserRpcError::Transport {
        message: error.to_string(),
        uncertain: false,
    })?;
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None).map_err(|error| {
        BrowserRpcError::Transport {
            message: error.to_string(),
            uncertain: false,
        }
    })?;
    let connect_timeout = timeout.min(Duration::from_secs(CONNECT_TIMEOUT_SECONDS));
    socket
        .connect_timeout(&address, connect_timeout)
        .map_err(|error| BrowserRpcError::Transport {
            message: error.to_string(),
            uncertain: false,
        })?;
    let mut channel: UnixStream = socket.into();
    let write_timeout = remaining(deadline).map_err(|_| BrowserRpcError::Protocol {
        message: "request exceeded its timeout before send",
        uncertain: false,
    })?;
    channel
        .set_write_timeout(Some(write_timeout))
        .map_err(|error| BrowserRpcError::Transport {
            message: error.to_string(),
            uncertain: false,
        })?;
    channel
        .write_all(&frame)
        .map_err(|error| BrowserRpcError::Transport {
            message: error.to_string(),
            uncertain: true,
        })?;

    let mut bytes = Vec::with_capacity(BUFFER_READ_BYTES);
    loop {
        let next = read_line(&mut channel, deadline, &mut bytes)?;
        let response: Value =
            studio_json::from_slice(&next).map_err(|_| BrowserRpcError::Protocol {
                message: "response is not valid JSON",
                uncertain: true,
            })?;
        let Some(fields) = response.as_object() else {
            return Err(BrowserRpcError::Protocol {
                message: "response must be an object",
                uncertain: true,
            });
        };
        if fields.get("id").and_then(Value::as_str) != Some(identity.as_str()) {
            return Err(BrowserRpcError::Protocol {
                message: "response identity differs from the request",
                uncertain: true,
            });
        }
        if let Some(error) = fields.get("error") {
            let (code, detail) = error_fields(error);
            return Err(BrowserRpcError::Rejected {
                uncertain: code.as_deref() == Some("outcome_unknown"),
                code,
                detail,
            });
        }
        on_frame(response.clone())
            .map_err(|_| BrowserRpcError::FrameCallback { uncertain: true })?;
        if let Some(result) = fields.get("result") {
            return Ok(result.clone());
        }
    }
}

#[cfg(unix)]
fn read_line(
    channel: &mut std::os::unix::net::UnixStream,
    deadline: Instant,
    buffer: &mut Vec<u8>,
) -> Result<Vec<u8>, BrowserRpcError> {
    // Darwin may reject SO_RCVTIMEO after the peer closes, even when its full
    // response is buffered. Poll an owned nonblocking socket instead, so both
    // buffered EOF responses and incomplete responses obey the same deadline.
    channel
        .set_nonblocking(true)
        .map_err(|error| BrowserRpcError::Transport {
            message: error.to_string(),
            uncertain: true,
        })?;
    let mut chunk = [0; BUFFER_READ_BYTES];
    loop {
        if let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            if end > FRAME_LIMIT_BYTES {
                return Err(BrowserRpcError::Protocol {
                    message: "response exceeds the frame limit",
                    uncertain: true,
                });
            }
            let remainder = buffer.split_off(end + 1);
            let mut line = std::mem::replace(buffer, remainder);
            line.pop();
            return Ok(line);
        }
        if buffer.len() > FRAME_LIMIT_BYTES {
            return Err(BrowserRpcError::Protocol {
                message: "response exceeds the frame limit",
                uncertain: true,
            });
        }
        let wait = remaining(deadline).map_err(|_| BrowserRpcError::Protocol {
            message: "response exceeded its timeout",
            uncertain: true,
        })?;
        let timeout = rustix::event::Timespec::try_from(wait).map_err(|error| {
            BrowserRpcError::Transport {
                message: error.to_string(),
                uncertain: true,
            }
        })?;
        let mut socket = rustix::event::PollFd::new(&*channel, rustix::event::PollFlags::IN);
        match rustix::event::poll(std::slice::from_mut(&mut socket), Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => continue,
            Ok(_) => {}
            Err(error) => {
                return Err(BrowserRpcError::Transport {
                    message: error.to_string(),
                    uncertain: true,
                });
            }
        }
        match channel.read(&mut chunk) {
            Ok(0) => {
                return Err(BrowserRpcError::Transport {
                    message: "connection closed before a result".to_owned(),
                    uncertain: true,
                });
            }
            Ok(length) => buffer.extend_from_slice(&chunk[..length]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(error) => {
                return Err(BrowserRpcError::Transport {
                    message: error.to_string(),
                    uncertain: true,
                });
            }
        }
    }
}

fn request_identity(request_id: Option<&str>) -> Result<String, BrowserRpcError> {
    if let Some(identity) = request_id {
        if identity.is_empty() || identity.chars().count() > REQUEST_ID_MAX_CHARS {
            return Err(BrowserRpcError::InvalidRequest);
        }
        return Ok(identity.to_owned());
    }
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|_| BrowserRpcError::Identity)?;
    Ok(data_encoding::HEXLOWER.encode(&random))
}

fn validate_request(
    method: &str,
    identity: &str,
    timeout: Duration,
) -> Result<(), BrowserRpcError> {
    if method.is_empty()
        || identity.is_empty()
        || identity.chars().count() > REQUEST_ID_MAX_CHARS
        || timeout.is_zero()
    {
        return Err(BrowserRpcError::InvalidRequest);
    }
    let maximum = RPC_TIMEOUT_SECONDS;
    if timeout > Duration::from_secs(maximum) {
        return Err(BrowserRpcError::InvalidRequest);
    }
    Ok(())
}

fn remaining(deadline: Instant) -> Result<Duration, BrowserRpcError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(BrowserRpcError::Protocol {
            message: "request exceeded its timeout",
            uncertain: true,
        })
}

fn error_fields(error: &Value) -> (Option<String>, Option<String>) {
    match error {
        Value::String(detail) => (None, Some(detail.clone())),
        Value::Object(fields) => (
            fields
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_owned),
            fields
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => (None, None),
    }
}
