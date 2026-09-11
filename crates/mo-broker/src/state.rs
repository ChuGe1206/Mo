use std::collections::BTreeMap;
use std::fmt;

use mo_ipc::{
    CURRENT_VERSION, ErrorMessage, FEATURE_KEY_EVENTS, FLAG_ERROR, FLAG_RESPONSE, Frame,
    FrameError, Hello, HelloAck, KeyEvent, MAX_COMPOSITION_BYTES, MAX_PAYLOAD_LEN, MessageKind,
    PayloadCodec, ProtocolVersion, Snapshot, VersionRange, negotiate_version,
};

pub const ERROR_BAD_REQUEST: u32 = 1;
pub const ERROR_BAD_HANDSHAKE: u32 = 2;
pub const ERROR_INCOMPATIBLE_VERSION: u32 = 3;
pub const ERROR_OUT_OF_ORDER: u32 = 4;
pub const ERROR_NO_SUCH_SESSION: u32 = 5;
pub const ERROR_SESSION_LIMIT: u32 = 6;

const MAX_SESSIONS_PER_CONNECTION: usize = 64;

#[derive(Debug, Default)]
struct SessionState {
    revision: u64,
    composition: String,
}

/// Transport-independent state for one authenticated transport connection.
///
/// `connection_generation` and session tokens are opaque correlation values,
/// not authentication secrets. Production authorization belongs to the future
/// named-pipe ACL and peer validation layer.
#[derive(Debug)]
pub struct BrokerConnection {
    connection_generation: u64,
    negotiated: Option<ProtocolVersion>,
    last_request_id: u64,
    next_session_token: u64,
    sessions: BTreeMap<u64, SessionState>,
}

impl BrokerConnection {
    pub fn new(connection_generation: u64) -> Self {
        Self {
            connection_generation: connection_generation.max(1),
            negotiated: None,
            last_request_id: 0,
            next_session_token: 1,
            sessions: BTreeMap::new(),
        }
    }

    pub fn connection_generation(&self) -> u64 {
        self.connection_generation
    }

    pub fn handle(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        if request.header.flags != 0 {
            return self.error(&request, ERROR_BAD_REQUEST, "request flags must be zero");
        }
        if request.header.request_id == 0 || request.header.request_id <= self.last_request_id {
            return self.error(
                &request,
                ERROR_OUT_OF_ORDER,
                "request_id must be non-zero and strictly increasing",
            );
        }
        self.last_request_id = request.header.request_id;

        if self.negotiated.is_none() {
            return self.handle_hello(request);
        }

        let selected = self.negotiated.expect("checked above");
        if request.header.version != selected {
            return self.error(
                &request,
                ERROR_INCOMPATIBLE_VERSION,
                "frame version differs from negotiated version",
            );
        }
        if request.header.connection_generation != self.connection_generation {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "connection generation is stale or invalid",
            );
        }

        match request.header.kind {
            MessageKind::OpenSession => self.open_session(request),
            MessageKind::KeyEvent => self.key_event(request),
            MessageKind::CloseSession => self.close_session(request),
            MessageKind::Ping => self.pong(request),
            _ => self.error(
                &request,
                ERROR_BAD_REQUEST,
                "message kind is not valid as a broker request",
            ),
        }
    }

    fn handle_hello(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        if request.header.kind != MessageKind::Hello
            || request.header.connection_generation != 0
            || request.header.session_token != 0
        {
            return self.error(
                &request,
                ERROR_BAD_HANDSHAKE,
                "first frame must be Hello with zero generation and session token",
            );
        }

        let hello = match Hello::decode_payload(&request.payload) {
            Ok(hello) => hello,
            Err(_) => {
                return self.error(&request, ERROR_BAD_HANDSHAKE, "Hello payload is malformed");
            }
        };
        let local = VersionRange::new(CURRENT_VERSION, CURRENT_VERSION)
            .expect("the built-in range is valid");
        let Some(selected) = negotiate_version(local, hello.supported) else {
            return self.error(
                &request,
                ERROR_INCOMPATIBLE_VERSION,
                "no compatible IPC protocol version",
            );
        };
        self.negotiated = Some(selected);

        let ack = HelloAck {
            selected,
            features: hello.features & FEATURE_KEY_EVENTS,
            max_payload_len: hello
                .max_payload_len
                .min(u32::try_from(MAX_PAYLOAD_LEN).expect("hard limit fits u32")),
        };
        self.response(
            selected,
            MessageKind::HelloAck,
            0,
            request.header.request_id,
            ack.encode_payload()?,
        )
    }

    fn open_session(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        if request.header.session_token != 0 || !request.payload.is_empty() {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "OpenSession requires an empty payload and zero session token",
            );
        }
        if self.sessions.len() >= MAX_SESSIONS_PER_CONNECTION {
            return self.error(
                &request,
                ERROR_SESSION_LIMIT,
                "connection session limit reached",
            );
        }

        let token = self.allocate_session_token();
        self.sessions.insert(token, SessionState::default());
        self.response(
            self.selected_version(),
            MessageKind::OpenSessionAck,
            token,
            request.header.request_id,
            Vec::new(),
        )
    }

    fn key_event(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        let token = request.header.session_token;
        let Some(session) = self.sessions.get_mut(&token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        let event = match KeyEvent::decode_payload(&request.payload) {
            Ok(event) => event,
            Err(_) => {
                return self.error(&request, ERROR_BAD_REQUEST, "KeyEvent payload is malformed");
            }
        };

        // This deterministic ASCII echo is a protocol spike, not an IME engine.
        let mut handled = false;
        let mut commit = None;
        if event.key_down {
            match event.virtual_key {
                0x41..=0x5a if session.composition.len() < MAX_COMPOSITION_BYTES => {
                    let character = char::from_u32(event.virtual_key + 0x20)
                        .expect("ASCII virtual key is a scalar value");
                    session.composition.push(character);
                    handled = true;
                }
                0x08 if !session.composition.is_empty() => {
                    session.composition.pop();
                    handled = true;
                }
                0x1b if !session.composition.is_empty() => {
                    session.composition.clear();
                    handled = true;
                }
                0x20 if !session.composition.is_empty() => {
                    commit = Some(std::mem::take(&mut session.composition));
                    handled = true;
                }
                _ => {}
            }
        }
        if handled {
            session.revision = session.revision.saturating_add(1);
        }
        let candidates = if session.composition.is_empty() {
            Vec::new()
        } else {
            vec![session.composition.clone()]
        };
        let snapshot = Snapshot {
            revision: session.revision,
            handled,
            composition: session.composition.clone(),
            commit,
            candidates,
        };
        self.response(
            self.selected_version(),
            MessageKind::Snapshot,
            token,
            request.header.request_id,
            snapshot.encode_payload()?,
        )
    }

    fn close_session(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        if !request.payload.is_empty() {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "CloseSession requires an empty payload",
            );
        }
        let token = request.header.session_token;
        if token == 0 || self.sessions.remove(&token).is_none() {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        }
        self.response(
            self.selected_version(),
            MessageKind::CloseSessionAck,
            token,
            request.header.request_id,
            Vec::new(),
        )
    }

    fn pong(&self, request: Frame) -> Result<Frame, BrokerError> {
        if request.header.session_token != 0 || !request.payload.is_empty() {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "Ping requires an empty payload and zero session token",
            );
        }
        self.response(
            self.selected_version(),
            MessageKind::Pong,
            0,
            request.header.request_id,
            Vec::new(),
        )
    }

    fn allocate_session_token(&mut self) -> u64 {
        loop {
            let candidate = self.next_session_token;
            self.next_session_token = self.next_session_token.wrapping_add(1).max(1);
            if candidate != 0 && !self.sessions.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn selected_version(&self) -> ProtocolVersion {
        self.negotiated.unwrap_or(CURRENT_VERSION)
    }

    fn response(
        &self,
        version: ProtocolVersion,
        kind: MessageKind,
        session_token: u64,
        request_id: u64,
        payload: Vec<u8>,
    ) -> Result<Frame, BrokerError> {
        Ok(Frame::new(
            version,
            kind,
            FLAG_RESPONSE,
            self.connection_generation,
            session_token,
            request_id,
            payload,
        )?)
    }

    fn error(
        &self,
        request: &Frame,
        code: u32,
        message: &'static str,
    ) -> Result<Frame, BrokerError> {
        let payload = ErrorMessage {
            code,
            message: message.to_owned(),
        }
        .encode_payload()?;
        Ok(Frame::new(
            self.selected_version(),
            MessageKind::Error,
            FLAG_RESPONSE | FLAG_ERROR,
            self.connection_generation,
            request.header.session_token,
            request.header.request_id,
            payload,
        )?)
    }
}

#[derive(Debug)]
pub enum BrokerError {
    Frame(FrameError),
    Codec(mo_ipc::CodecError),
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "could not encode broker frame: {error}"),
            Self::Codec(error) => write!(formatter, "could not encode broker payload: {error}"),
        }
    }
}

impl std::error::Error for BrokerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::Codec(error) => Some(error),
        }
    }
}

impl From<FrameError> for BrokerError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

impl From<mo_ipc::CodecError> for BrokerError {
    fn from(value: mo_ipc::CodecError) -> Self {
        Self::Codec(value)
    }
}
