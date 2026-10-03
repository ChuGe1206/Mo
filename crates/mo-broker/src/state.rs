use std::collections::BTreeMap;
use std::fmt;

use mo_domain::{
    EngineCommand, KeyEvent as DomainKeyEvent, KeyModifiers, KeyState, SessionOptions, SessionToken,
};
use mo_engine::{EngineActor, EngineBackend, FakeBackend};
use mo_ipc::{
    CURRENT_VERSION, CandidateAction, CandidateActionKind, CharacterSet as WireCharacterSet,
    DetailedCandidate, DetailedSnapshot, ErrorMessage, FEATURE_CANDIDATE_ACTIONS,
    FEATURE_CANDIDATE_DETAILS, FEATURE_KEY_EVENTS, FEATURE_SESSION_SETTINGS_ACK,
    FEATURE_SETTINGS_SNAPSHOT, FLAG_ERROR, FLAG_RESPONSE, Frame, FrameError, Hello, HelloAck,
    InputScheme as WireInputScheme, KeyEvent as WireKeyEvent, MAX_CANDIDATE_COMMENT_BYTES,
    MAX_CANDIDATE_LABEL_BYTES, MAX_PAYLOAD_LEN, MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN, MessageKind,
    PayloadCodec, ProtocolVersion, SettingsSnapshot as WireSettingsSnapshot, Snapshot,
    VersionRange, negotiate_version,
};

use crate::engine_service::EngineClient;
use crate::settings_service::SettingsService;

pub const ERROR_BAD_REQUEST: u32 = 1;
pub const ERROR_BAD_HANDSHAKE: u32 = 2;
pub const ERROR_INCOMPATIBLE_VERSION: u32 = 3;
pub const ERROR_OUT_OF_ORDER: u32 = 4;
pub const ERROR_NO_SUCH_SESSION: u32 = 5;
pub const ERROR_SESSION_LIMIT: u32 = 6;
pub const ERROR_ENGINE_FAILURE: u32 = 7;
pub const ERROR_STALE_CANDIDATES: u32 = 8;
pub const ERROR_SETTINGS_UNAVAILABLE: u32 = 9;

const MAX_SESSIONS_PER_CONNECTION: usize = 64;

/// Transport-independent state for one authenticated transport connection.
///
/// `connection_generation` and session tokens are opaque correlation values,
/// not authentication secrets. Transport authorization belongs to the adapter.
/// The local constructor owns an Engine Actor directly for deterministic tests.
/// The production listener gives every connection a clone of a process-wide
/// engine client, preserving this connection's wire-token ownership while all
/// backend calls remain serialized on the engine thread.
pub struct BrokerConnection<B = FakeBackend>
where
    B: EngineBackend,
{
    connection_generation: u64,
    negotiated: Option<ProtocolVersion>,
    negotiated_features: u64,
    last_request_id: u64,
    next_session_token: u64,
    sessions: BTreeMap<u64, WireSession>,
    engine: EngineOwner<B>,
    settings: SettingsService,
}

struct WireSession {
    engine_token: SessionToken,
    /// Only successfully encoded pages authorize a subsequent UI action.
    page: Option<(u64, usize)>,
}

enum EngineOwner<B>
where
    B: EngineBackend,
{
    Local(EngineActor<B>),
    Shared(EngineClient),
}

impl<B> EngineOwner<B>
where
    B: EngineBackend,
{
    fn create_session(
        &mut self,
        options: SessionOptions,
    ) -> Result<SessionToken, EngineOperationError> {
        match self {
            Self::Local(actor) => actor
                .create_session(options)
                .map_err(|_| EngineOperationError),
            Self::Shared(client) => client
                .create_session(options)
                .map_err(|_| EngineOperationError),
        }
    }

    fn dispatch(
        &mut self,
        token: SessionToken,
        command: EngineCommand,
    ) -> Result<mo_domain::EngineSnapshot, EngineOperationError> {
        match self {
            Self::Local(actor) => actor
                .dispatch(token, command)
                .map_err(|_| EngineOperationError),
            Self::Shared(client) => client
                .dispatch(token, command)
                .map_err(|_| EngineOperationError),
        }
    }

    fn destroy_session(&mut self, token: SessionToken) -> Result<(), EngineOperationError> {
        match self {
            Self::Local(actor) => actor
                .destroy_session(token)
                .map_err(|_| EngineOperationError),
            Self::Shared(client) => client
                .destroy_session(token)
                .map_err(|_| EngineOperationError),
        }
    }
}

struct EngineOperationError;

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
            negotiated_features: 0,
            last_request_id: 0,
            next_session_token: 1,
            sessions: BTreeMap::new(),
            engine: EngineOwner::Local(EngineActor::new(backend)),
            settings: SettingsService::defaults(),
        }
    }

    pub(crate) fn with_engine_client(
        connection_generation: u64,
        engine: EngineClient,
        settings: SettingsService,
    ) -> Self {
        Self {
            connection_generation: connection_generation.max(1),
            negotiated: None,
            negotiated_features: 0,
            last_request_id: 0,
            next_session_token: 1,
            sessions: BTreeMap::new(),
            engine: EngineOwner::Shared(engine),
            settings,
        }
    }

    pub fn connection_generation(&self) -> u64 {
        self.connection_generation
    }

    pub fn engine(&self) -> &EngineActor<B> {
        match &self.engine {
            EngineOwner::Local(actor) => actor,
            EngineOwner::Shared(_) => {
                unreachable!("shared production engine is not exposed through BrokerConnection")
            }
        }
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
            MessageKind::CandidateAction => self.candidate_action(request),
            MessageKind::GetSettings => self.get_settings(request),
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
        self.negotiated_features = hello.features
            & (FEATURE_KEY_EVENTS
                | FEATURE_CANDIDATE_ACTIONS
                | FEATURE_CANDIDATE_DETAILS
                | FEATURE_SETTINGS_SNAPSHOT
                | FEATURE_SESSION_SETTINGS_ACK);
        if hello.max_payload_len < MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN as u32 {
            self.negotiated_features &= !FEATURE_CANDIDATE_DETAILS;
        }

        let ack = HelloAck {
            selected,
            features: self.negotiated_features,
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

        let needs_settings = matches!(self.engine, EngineOwner::Shared(_))
            || self.negotiated_features & FEATURE_SESSION_SETTINGS_ACK != 0;
        let settings_snapshot = if needs_settings {
            let settings = match self.settings.snapshot() {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    return self.error(
                        &request,
                        ERROR_SETTINGS_UNAVAILABLE,
                        "settings document is unavailable",
                    );
                }
            };
            Some(settings)
        } else {
            None
        };
        let options = if matches!(self.engine, EngineOwner::Shared(_)) {
            session_options_from_settings(settings_snapshot.expect("shared engine reads settings"))
        } else {
            SessionOptions::new()
        };
        let payload = if self.negotiated_features & FEATURE_SESSION_SETTINGS_ACK != 0 {
            settings_snapshot
                .expect("negotiated session settings were read")
                .encode_payload()?
        } else {
            Vec::new()
        };
        let engine_token = match self.engine.create_session(options) {
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
        self.sessions.insert(
            token,
            WireSession {
                engine_token,
                page: None,
            },
        );
        self.response(
            self.selected_version(),
            MessageKind::OpenSessionAck,
            token,
            request.header.request_id,
            payload,
        )
    }

    fn key_event(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        let token = request.header.session_token;
        let Some(session) = self.sessions.get(&token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        let engine_token = session.engine_token;
        let event = match WireKeyEvent::decode_payload(&request.payload) {
            Ok(event) => event,
            Err(_) => {
                return self.error(&request, ERROR_BAD_REQUEST, "KeyEvent payload is malformed");
            }
        };
        self.dispatch_snapshot(
            &request,
            engine_token,
            EngineCommand::Key(normalize_key_event(event)),
        )
    }

    fn candidate_action(&mut self, request: Frame) -> Result<Frame, BrokerError> {
        if self.negotiated_features & FEATURE_CANDIDATE_ACTIONS == 0 {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "candidate actions were not negotiated",
            );
        }
        let Some(session) = self.sessions.get(&request.header.session_token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        let action = match CandidateAction::decode_payload(&request.payload) {
            Ok(action) => action,
            Err(_) => {
                return self.error(
                    &request,
                    ERROR_BAD_REQUEST,
                    "CandidateAction payload is malformed",
                );
            }
        };
        let Some((revision, count)) = session.page else {
            return self.error(
                &request,
                ERROR_STALE_CANDIDATES,
                "no displayed candidate page is current",
            );
        };
        if action.expected_revision != revision {
            return self.error(
                &request,
                ERROR_STALE_CANDIDATES,
                "displayed candidate revision is stale",
            );
        }
        let command = match action.action {
            CandidateActionKind::Select => {
                if action.index as usize >= count {
                    return self.error(
                        &request,
                        ERROR_BAD_REQUEST,
                        "candidate ordinal is outside the displayed page",
                    );
                }
                EngineCommand::SelectCandidate {
                    index: action.index,
                }
            }
            CandidateActionKind::PreviousPage => EngineCommand::ChangePage { backward: true },
            CandidateActionKind::NextPage => EngineCommand::ChangePage { backward: false },
        };
        let engine_token = session.engine_token;
        self.dispatch_snapshot(&request, engine_token, command)
    }

    fn dispatch_snapshot(
        &mut self,
        request: &Frame,
        engine_token: SessionToken,
        command: EngineCommand,
    ) -> Result<Frame, BrokerError> {
        let token = request.header.session_token;
        // A backend error may follow a native state change. Never authorize an
        // old UI page after an attempted command, even when encoding fails.
        self.sessions
            .get_mut(&token)
            .expect("validated session")
            .page = None;
        let engine_snapshot = match self.engine.dispatch(engine_token, command) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return self.error(
                    request,
                    ERROR_ENGINE_FAILURE,
                    "engine rejected the input command",
                );
            }
        };
        let revision = engine_snapshot.revision.get();
        let handled = engine_snapshot.handled;
        let composition = engine_snapshot
            .composition
            .map(|composition| composition.preedit().to_owned())
            .unwrap_or_default();
        let has_composition = !composition.is_empty();
        let commit = engine_snapshot.commit;
        let candidate_count = engine_snapshot.candidates.len();
        let (kind, payload) = if self.negotiated_features & FEATURE_CANDIDATE_DETAILS != 0 {
            let detailed = DetailedSnapshot {
                revision,
                handled,
                composition,
                commit,
                candidates: engine_snapshot
                    .candidates
                    .into_iter()
                    .map(|candidate| DetailedCandidate {
                        text: candidate.text,
                        comment: candidate
                            .comment
                            .filter(|value| value.len() <= MAX_CANDIDATE_COMMENT_BYTES),
                        label: candidate
                            .label
                            .filter(|value| value.len() <= MAX_CANDIDATE_LABEL_BYTES),
                    })
                    .collect(),
            };
            (MessageKind::DetailedSnapshot, detailed.encode_payload()?)
        } else {
            let legacy = Snapshot {
                revision,
                handled,
                composition,
                commit,
                candidates: engine_snapshot
                    .candidates
                    .into_iter()
                    .map(|candidate| candidate.text)
                    .collect(),
            };
            (MessageKind::Snapshot, legacy.encode_payload()?)
        };
        if has_composition && candidate_count != 0 {
            self.sessions
                .get_mut(&token)
                .expect("validated session")
                .page = Some((revision, candidate_count));
        }
        self.response(
            self.selected_version(),
            kind,
            token,
            request.header.request_id,
            payload,
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
        let Some(session) = self.sessions.remove(&token) else {
            return self.error(
                &request,
                ERROR_NO_SUCH_SESSION,
                "session token does not belong to this connection",
            );
        };
        if self.engine.destroy_session(session.engine_token).is_err() {
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

    fn get_settings(&self, request: Frame) -> Result<Frame, BrokerError> {
        if self.negotiated_features & FEATURE_SETTINGS_SNAPSHOT == 0 {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "settings snapshots were not negotiated",
            );
        }
        if request.header.session_token != 0 || !request.payload.is_empty() {
            return self.error(
                &request,
                ERROR_BAD_REQUEST,
                "GetSettings requires an empty payload and zero session token",
            );
        }
        let snapshot = match self.settings.snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return self.error(
                    &request,
                    ERROR_SETTINGS_UNAVAILABLE,
                    "settings are unavailable; the last valid runtime plan was preserved",
                );
            }
        };
        self.response(
            self.selected_version(),
            MessageKind::SettingsSnapshot,
            0,
            request.header.request_id,
            snapshot.encode_payload()?,
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

fn session_options_from_settings(settings: WireSettingsSnapshot) -> SessionOptions {
    let schema = match settings.input_scheme {
        WireInputScheme::FullPinyin => "rime_ice",
        WireInputScheme::DoublePinyinNatural => "double_pinyin",
        WireInputScheme::DoublePinyinFlypy => "double_pinyin_flypy",
        WireInputScheme::DoublePinyinMicrosoft => "double_pinyin_mspy",
        WireInputScheme::DoublePinyinSogou => "double_pinyin_sogou",
    };
    SessionOptions::new()
        .with_schema(schema)
        .with_option(
            "traditionalization",
            settings.character_set == WireCharacterSet::Traditional,
        )
        .with_option("emoji", settings.emoji)
        .with_option("mo_disable_learning", !settings.effective_learning)
}

#[cfg(test)]
mod session_settings_tests {
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use mo_domain::{EngineOutput, SessionOptions};
    use mo_ipc::{ErrorMessage, KeyEvent, PayloadCodec};
    use mo_settings::{CharacterSet, InputScheme, Settings, save_atomic};

    use crate::engine_service::EngineService;

    use super::*;

    struct RecordingBackend {
        inner: FakeBackend,
        options: Arc<Mutex<Vec<SessionOptions>>>,
        reject_next_create: Arc<AtomicBool>,
    }

    impl EngineBackend for RecordingBackend {
        type Session = <FakeBackend as EngineBackend>::Session;
        type Error = <FakeBackend as EngineBackend>::Error;

        fn create_session(
            &mut self,
            options: SessionOptions,
        ) -> Result<Self::Session, Self::Error> {
            if self.reject_next_create.swap(false, Ordering::SeqCst) {
                return Err(mo_engine::FakeError::SessionIdExhausted);
            }
            self.options.lock().unwrap().push(options.clone());
            self.inner.create_session(options)
        }

        fn apply(
            &mut self,
            session: &mut Self::Session,
            command: &EngineCommand,
        ) -> Result<EngineOutput, Self::Error> {
            self.inner.apply(session, command)
        }

        fn destroy_session(&mut self, session: Self::Session) -> Result<(), Self::Error> {
            self.inner.destroy_session(session)
        }
    }

    #[test]
    fn pinned_settings_map_to_librime_session_options() {
        let defaults = SettingsService::defaults().snapshot().unwrap();
        for (scheme, schema) in [
            (WireInputScheme::FullPinyin, "rime_ice"),
            (WireInputScheme::DoublePinyinNatural, "double_pinyin"),
            (WireInputScheme::DoublePinyinFlypy, "double_pinyin_flypy"),
            (WireInputScheme::DoublePinyinMicrosoft, "double_pinyin_mspy"),
            (WireInputScheme::DoublePinyinSogou, "double_pinyin_sogou"),
        ] {
            for (character_set, traditional) in [
                (WireCharacterSet::Simplified, false),
                (WireCharacterSet::Traditional, true),
            ] {
                for emoji in [false, true] {
                    for effective_learning in [false, true] {
                        let options = session_options_from_settings(WireSettingsSnapshot {
                            input_scheme: scheme,
                            character_set,
                            emoji,
                            local_learning: effective_learning,
                            privacy_mode: false,
                            effective_learning,
                            ..defaults
                        });
                        assert_eq!(options.schema_id.as_deref(), Some(schema));
                        assert_eq!(options.options.len(), 3);
                        assert_eq!(
                            options.options.get("traditionalization"),
                            Some(&traditional)
                        );
                        assert_eq!(options.options.get("emoji"), Some(&emoji));
                        assert_eq!(
                            options.options.get("mo_disable_learning"),
                            Some(&!effective_learning)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn learning_preference_and_privacy_mode_both_disable_session_learning() {
        let defaults = SettingsService::defaults().snapshot().unwrap();
        for (local_learning, privacy_mode, expected_disable) in [
            (true, false, false),
            (false, false, true),
            (true, true, true),
            (false, true, true),
        ] {
            let options = session_options_from_settings(WireSettingsSnapshot {
                local_learning,
                privacy_mode,
                effective_learning: local_learning && !privacy_mode,
                ..defaults
            });
            assert_eq!(
                options.options.get("mo_disable_learning"),
                Some(&expected_disable)
            );
        }
    }

    #[test]
    fn opens_new_settings_session_before_retiring_old_and_rejection_preserves_old() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mo-session-replace-{}-{nonce}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("settings.mo");
        let options = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&options);
        let reject_next_create = Arc::new(AtomicBool::new(false));
        let reject_in_engine = Arc::clone(&reject_next_create);
        let engine = EngineService::start(move || {
            Ok::<_, &'static str>(RecordingBackend {
                inner: FakeBackend::new(),
                options: observed,
                reject_next_create: reject_in_engine,
            })
        })
        .unwrap();
        let mut connection = BrokerConnection::<FakeBackend>::with_engine_client(
            42,
            engine.client(),
            SettingsService::open(path.clone()).unwrap(),
        );
        let request = |kind, token, id, payload| {
            Frame::new(
                CURRENT_VERSION,
                kind,
                0,
                if kind == MessageKind::Hello { 0 } else { 42 },
                token,
                id,
                payload,
            )
            .unwrap()
        };
        let hello = Hello {
            supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
            features: FEATURE_KEY_EVENTS | FEATURE_SETTINGS_SNAPSHOT | FEATURE_SESSION_SETTINGS_ACK,
            max_payload_len: MAX_PAYLOAD_LEN as u32,
        };
        connection
            .handle(request(
                MessageKind::Hello,
                0,
                1,
                hello.encode_payload().unwrap(),
            ))
            .unwrap();
        let old = connection
            .handle(request(MessageKind::OpenSession, 0, 2, Vec::new()))
            .unwrap();
        assert_eq!(old.header.kind, MessageKind::OpenSessionAck);
        let old_settings = WireSettingsSnapshot::decode_payload(&old.payload).unwrap();
        assert_eq!(old_settings.input_scheme, WireInputScheme::FullPinyin);
        assert_eq!(old_settings.character_set, WireCharacterSet::Simplified);
        let old_token = old.header.session_token;

        fs::write(&path, b"broken").unwrap();
        let rejected = connection
            .handle(request(MessageKind::OpenSession, 0, 3, Vec::new()))
            .unwrap();
        assert_eq!(
            ErrorMessage::decode_payload(&rejected.payload)
                .unwrap()
                .code,
            ERROR_SETTINGS_UNAVAILABLE
        );
        let key = KeyEvent {
            virtual_key: 0x4e,
            scan_code: 0,
            modifiers: 0,
            key_down: true,
            repeat: false,
        };
        let old_key = connection
            .handle(request(
                MessageKind::KeyEvent,
                old_token,
                4,
                key.encode_payload().unwrap(),
            ))
            .unwrap();
        assert_eq!(old_key.header.kind, MessageKind::Snapshot);
        assert_eq!(
            Snapshot::decode_payload(&old_key.payload)
                .unwrap()
                .composition,
            "n"
        );

        save_atomic(
            &path,
            &Settings {
                input_scheme: InputScheme::DoublePinyinFlypy,
                character_set: CharacterSet::Traditional,
                emoji: false,
                ..Settings::default()
            },
        )
        .unwrap();
        reject_next_create.store(true, Ordering::SeqCst);
        let rejected = connection
            .handle(request(MessageKind::OpenSession, 0, 5, Vec::new()))
            .unwrap();
        assert_eq!(
            ErrorMessage::decode_payload(&rejected.payload)
                .unwrap()
                .code,
            ERROR_ENGINE_FAILURE
        );
        let old_key = connection
            .handle(request(
                MessageKind::KeyEvent,
                old_token,
                6,
                key.encode_payload().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            Snapshot::decode_payload(&old_key.payload)
                .unwrap()
                .composition,
            "nn"
        );
        let new = connection
            .handle(request(MessageKind::OpenSession, 0, 7, Vec::new()))
            .unwrap();
        assert_eq!(new.header.kind, MessageKind::OpenSessionAck);
        let applied_settings = WireSettingsSnapshot::decode_payload(&new.payload).unwrap();
        assert!(applied_settings.revision > old_settings.revision);
        assert_eq!(
            applied_settings.input_scheme,
            WireInputScheme::DoublePinyinFlypy
        );
        assert_eq!(
            applied_settings.character_set,
            WireCharacterSet::Traditional
        );
        assert!(!applied_settings.emoji);
        assert_ne!(new.header.session_token, old_token);
        let closed = connection
            .handle(request(MessageKind::CloseSession, old_token, 8, Vec::new()))
            .unwrap();
        assert_eq!(closed.header.kind, MessageKind::CloseSessionAck);
        let new_key = connection
            .handle(request(
                MessageKind::KeyEvent,
                new.header.session_token,
                9,
                key.encode_payload().unwrap(),
            ))
            .unwrap();
        assert_eq!(new_key.header.kind, MessageKind::Snapshot);
        assert_eq!(
            Snapshot::decode_payload(&new_key.payload)
                .unwrap()
                .composition,
            "n"
        );
        let captured = options.lock().unwrap();
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].schema_id.as_deref(), Some("rime_ice"));
        assert_eq!(
            captured[1].schema_id.as_deref(),
            Some("double_pinyin_flypy")
        );
        assert_eq!(captured[1].options.get("traditionalization"), Some(&true));
        assert_eq!(captured[0].options.get("emoji"), Some(&true));
        assert_eq!(captured[1].options.get("emoji"), Some(&false));
        drop(captured);
        drop(connection);
        engine.shutdown().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
}

impl<B> Drop for BrokerConnection<B>
where
    B: EngineBackend,
{
    fn drop(&mut self) {
        for session in std::mem::take(&mut self.sessions).into_values() {
            let _ = self.engine.destroy_session(session.engine_token);
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
        0x21 => 0xff55, // XK_Page_Up
        0x22 => 0xff56, // XK_Page_Down
        0x23 => 0xff57, // XK_End
        0x24 => 0xff50, // XK_Home
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
