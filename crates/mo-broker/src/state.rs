use std::collections::BTreeMap;
use std::fmt;

use mo_domain::{
    EngineCommand, KeyEvent as DomainKeyEvent, KeyModifiers, KeyState, SessionOptions, SessionToken,
};
use mo_engine::{EngineActor, EngineBackend, FakeBackend};
use mo_ipc::{
    CURRENT_VERSION, ErrorMessage, FEATURE_KEY_EVENTS, FLAG_ERROR, FLAG_RESPONSE, Frame,
    FrameError, Hello, HelloAck, KeyEvent as WireKeyEvent, MAX_PAYLOAD_LEN, MessageKind,
    PayloadCodec, ProtocolVersion, Snapshot, VersionRange, negotiate_version,
};

pub const ERROR_BAD_REQUEST: u32 = 1;
pub const ERROR_BAD_HANDSHAKE: u32 = 2;
pub const ERROR_INCOMPATIBLE_VERSION: u32 = 3;
pub const ERROR_OUT_OF_ORDER: u32 = 4;
pub const ERROR_NO_SUCH_SESSION: u32 = 5;
pub const ERROR_SESSION_LIMIT: u32 = 6;
pub const ERROR_ENGINE_FAILURE: u32 = 7;

const MAX_SESSIONS_PER_CONNECTION: usize = 64;

/// Transport-independent state for one authenticated transport connection.
///
/// `connection_generation` and session tokens are opaque correlation values,
/// not authentication secrets. Transport authorization belongs to the adapter.
/// Phase 0 serves one connection, so this type owns its Engine Actor directly;
/// a future listener pool will put the actor behind a process-wide command
/// channel without changing the wire-token mapping in this state machine.
pub struct BrokerConnection<B = FakeBackend>
where
    B: EngineBackend,
{
    connection_generation: u64,
    negotiated: Option<ProtocolVersion>,
    last_request_id: u64,
    next_session_token: u64,
    sessions: BTreeMap<u64, SessionToken>,
    engine: EngineActor<B>,
}

impl BrokerConnection<FakeBackend> {
    pub fn new(connection_generation: u64) -> Self {
        Self::with_backend(connection_generation, FakeBackend::new())
    }
}

impl<B> BrokerConnection<B>
where
    B: EngineBackend,
{
    pub fn with_backend(connection_generation: u64, backend: B) -> Self {
        Self {
            connection_generation: connection_generation.max(1),
            negotiated: None,
            last_request_id: 0,
            next_session_token: 1,
            sessions: BTreeMap::new(),
            engine: EngineActor::new(backend),
        }
    }

    pub fn connection_generation(&self) -> u64 {
        self.connection_generation
    }

    pub fn engine(&self) -> &EngineActor<B> {
        &self.engine
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

        let engine_token = match self.engine.create_session(SessionOptions::new()) {
            Ok(token) => token,
            Err(_) => {
                return self.error(
                    &request,
                    ERROR_ENGINE_FAILURE,
                    "engine could not create a session",
                );
            }
        };
        let token = self.allocate_session_token();
        self.sessions.insert(token, engine_token);
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
        let Some(&engine_token) = self.sessions.get(&token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        let event = match WireKeyEvent::decode_payload(&request.payload) {
            Ok(event) => event,
            Err(_) => {
                return self.error(&request, ERROR_BAD_REQUEST, "KeyEvent payload is malformed");
            }
        };
        let engine_snapshot = match self
            .engine
            .dispatch(engine_token, EngineCommand::Key(normalize_key_event(event)))
        {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return self.error(
                    &request,
                    ERROR_ENGINE_FAILURE,
                    "engine rejected the key event",
                );
            }
        };
        let snapshot = Snapshot {
            revision: engine_snapshot.revision.get(),
            handled: engine_snapshot.handled,
            composition: engine_snapshot
                .composition
                .map(|composition| composition.preedit().to_owned())
                .unwrap_or_default(),
            commit: engine_snapshot.commit,
            candidates: engine_snapshot
                .candidates
                .into_iter()
                .map(|candidate| candidate.text)
                .collect(),
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
        let Some(engine_token) = self.sessions.remove(&token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        if self.engine.destroy_session(engine_token).is_err() {
            return self.error(
                &request,
                ERROR_ENGINE_FAILURE,
                "engine could not destroy the session",
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

impl<B> Drop for BrokerConnection<B>
where
    B: EngineBackend,
{
    fn drop(&mut self) {
        for token in std::mem::take(&mut self.sessions).into_values() {
            let _ = self.engine.destroy_session(token);
        }
    }
}

fn normalize_key_event(event: WireKeyEvent) -> DomainKeyEvent {
    let modifiers = KeyModifiers::from_bits(u32::from(event.modifiers));
    let state = if event.key_down {
        KeyState::Pressed
    } else {
        KeyState::Released
    };
    let keycode = normalized_keycode(event.virtual_key, modifiers);
    let text = infer_ascii_text(keycode, state, modifiers);
    DomainKeyEvent::new(keycode, modifiers, state, text, event.repeat)
}

fn normalized_keycode(virtual_key: u32, modifiers: KeyModifiers) -> u32 {
    match virtual_key {
        0x08 => 0xff08, // XK_BackSpace
        0x09 => 0xff09, // XK_Tab
        0x0d => 0xff0d, // XK_Return
        0x1b => 0xff1b, // XK_Escape
        0x25 => 0xff51, // XK_Left
        0x26 => 0xff52, // XK_Up
        0x27 => 0xff53, // XK_Right
        0x28 => 0xff54, // XK_Down
        0x2e => 0xffff, // XK_Delete
        0x41..=0x5a => {
            let uppercase = modifiers.contains(KeyModifiers::SHIFT)
                ^ modifiers.contains(KeyModifiers::CAPS_LOCK);
            if uppercase {
                virtual_key
            } else {
                virtual_key + u32::from(b'a' - b'A')
            }
        }
        _ => virtual_key,
    }
}

fn infer_ascii_text(keycode: u32, state: KeyState, modifiers: KeyModifiers) -> Option<char> {
    if state != KeyState::Pressed
        || modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return None;
    }

    match keycode {
        0x61..=0x7a | 0x41..=0x5a => char::from_u32(keycode),
        0x30..=0x39 if !modifiers.contains(KeyModifiers::SHIFT) => char::from_u32(keycode),
        _ => None,
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
