use mo_broker::{
    BrokerConnection, ERROR_BAD_HANDSHAKE, ERROR_ENGINE_FAILURE, ERROR_NO_SUCH_SESSION,
    ERROR_OUT_OF_ORDER,
};
use mo_domain::{EngineCommand, EngineOutput, SessionOptions};
use mo_engine::{EngineBackend, FakeEvent};
use mo_ipc::{
    CURRENT_VERSION, ErrorMessage, FEATURE_KEY_EVENTS, FLAG_ERROR, FLAG_RESPONSE, Frame, Hello,
    HelloAck, KeyEvent, MAX_PAYLOAD_LEN, MessageKind, PayloadCodec, Snapshot, VersionRange,
};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::Duration;

fn frame(
    kind: MessageKind,
    generation: u64,
    token: u64,
    request_id: u64,
    payload: Vec<u8>,
) -> Frame {
    Frame::new(
        CURRENT_VERSION,
        kind,
        0,
        generation,
        token,
        request_id,
        payload,
    )
    .unwrap()
}

fn hello<B: EngineBackend>(connection: &mut BrokerConnection<B>, request_id: u64) -> Frame {
    let payload = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: FEATURE_KEY_EVENTS,
        max_payload_len: u32::try_from(MAX_PAYLOAD_LEN).unwrap(),
    }
    .encode_payload()
    .unwrap();
    connection
        .handle(frame(MessageKind::Hello, 0, 0, request_id, payload))
        .unwrap()
}

fn open<B: EngineBackend>(connection: &mut BrokerConnection<B>, request_id: u64) -> u64 {
    let response = connection
        .handle(frame(
            MessageKind::OpenSession,
            connection.connection_generation(),
            0,
            request_id,
            Vec::new(),
        ))
        .unwrap();
    assert_eq!(response.header.kind, MessageKind::OpenSessionAck);
    response.header.session_token
}

fn error_code(frame: Frame) -> u32 {
    assert_eq!(frame.header.kind, MessageKind::Error);
    assert_eq!(frame.header.flags, FLAG_RESPONSE | FLAG_ERROR);
    ErrorMessage::decode_payload(&frame.payload).unwrap().code
}

#[derive(Default)]
struct RejectKeyBackend;

impl EngineBackend for RejectKeyBackend {
    type Session = ();
    type Error = &'static str;

    fn create_session(&mut self, _options: SessionOptions) -> Result<Self::Session, Self::Error> {
        Ok(())
    }

    fn apply(
        &mut self,
        _session: &mut Self::Session,
        _command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error> {
        Err("injected key failure")
    }
}

#[test]
fn hello_negotiates_bounded_features_and_assigns_generation() {
    let mut connection = BrokerConnection::new(72);
    let response = hello(&mut connection, 1);
    assert_eq!(response.header.kind, MessageKind::HelloAck);
    assert_eq!(response.header.flags, FLAG_RESPONSE);
    assert_eq!(response.header.connection_generation, 72);
    let ack = HelloAck::decode_payload(&response.payload).unwrap();
    assert_eq!(ack.selected, CURRENT_VERSION);
    assert_eq!(ack.features, FEATURE_KEY_EVENTS);
}

#[test]
fn non_hello_first_frame_is_rejected() {
    let mut connection = BrokerConnection::new(1);
    let response = connection
        .handle(frame(MessageKind::Ping, 0, 0, 1, Vec::new()))
        .unwrap();
    assert_eq!(error_code(response), ERROR_BAD_HANDSHAKE);
}

#[test]
fn request_ids_must_be_strictly_increasing() {
    let mut connection = BrokerConnection::new(5);
    hello(&mut connection, 10);
    let response = connection
        .handle(frame(MessageKind::Ping, 5, 0, 10, Vec::new()))
        .unwrap();
    assert_eq!(error_code(response), ERROR_OUT_OF_ORDER);
}

#[test]
fn engine_backend_failure_is_a_stable_protocol_error() {
    let mut connection = BrokerConnection::with_backend(6, RejectKeyBackend);
    hello(&mut connection, 1);
    let token = open(&mut connection, 2);
    let event = KeyEvent {
        virtual_key: 0x4d,
        scan_code: 0,
        modifiers: 0,
        key_down: true,
        repeat: false,
    };
    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            6,
            token,
            3,
            event.encode_payload().unwrap(),
        ))
        .unwrap();

    assert_eq!(error_code(response), ERROR_ENGINE_FAILURE);
}

#[test]
fn candidate_navigation_keys_are_x11_keysyms_not_windows_virtual_keys() {
    let mut connection = BrokerConnection::new(13);
    hello(&mut connection, 1);
    let token = open(&mut connection, 2);
    for (index, (virtual_key, expected)) in [
        (0x21, 0xff55),
        (0x22, 0xff56),
        (0x23, 0xff57),
        (0x24, 0xff50),
    ]
    .into_iter()
    .enumerate()
    {
        let event = KeyEvent {
            virtual_key,
            scan_code: 0,
            modifiers: 0,
            key_down: true,
            repeat: false,
        };
        connection
            .handle(frame(
                MessageKind::KeyEvent,
                13,
                token,
                index as u64 + 3,
                event.encode_payload().unwrap(),
            ))
            .unwrap();
        let applied = connection
            .engine()
            .backend()
            .events()
            .iter()
            .rev()
            .find_map(|event| {
                if let FakeEvent::CommandApplied {
                    command: EngineCommand::Key(key),
                    ..
                } = event
                {
                    Some(key)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(applied.keycode, expected);
        assert_eq!(applied.text, None);
    }
}

#[test]
fn engine_actor_keeps_sessions_isolated_and_globally_orders_snapshots() {
    let mut connection = BrokerConnection::new(9);
    hello(&mut connection, 1);
    let first = open(&mut connection, 2);
    let second = open(&mut connection, 3);
    assert_ne!(first, second);

    let event = KeyEvent {
        virtual_key: 0x4d,
        scan_code: 0,
        modifiers: 0,
        key_down: true,
        repeat: false,
    };
    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            9,
            first,
            4,
            event.encode_payload().unwrap(),
        ))
        .unwrap();
    let snapshot = Snapshot::decode_payload(&response.payload).unwrap();
    assert!(snapshot.handled);
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.composition, "m");
    assert_eq!(snapshot.candidates, ["m", "M"]);

    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            9,
            second,
            5,
            event.encode_payload().unwrap(),
        ))
        .unwrap();
    let snapshot = Snapshot::decode_payload(&response.payload).unwrap();
    assert_eq!(snapshot.revision, 2);
    assert_eq!(snapshot.composition, "m");

    let applied_sessions = connection
        .engine()
        .backend()
        .events()
        .iter()
        .filter_map(|event| match event {
            FakeEvent::CommandApplied {
                backend_session, ..
            } => Some(*backend_session),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(applied_sessions, [1, 2]);
}

#[test]
fn space_commits_diagnostic_composition_and_close_revokes_token() {
    let mut connection = BrokerConnection::new(11);
    hello(&mut connection, 1);
    let token = open(&mut connection, 2);
    for (request_id, virtual_key) in [(3, 0x4d), (4, 0x4f), (5, 0x20)] {
        let event = KeyEvent {
            virtual_key,
            scan_code: 0,
            modifiers: 0,
            key_down: true,
            repeat: false,
        };
        let response = connection
            .handle(frame(
                MessageKind::KeyEvent,
                11,
                token,
                request_id,
                event.encode_payload().unwrap(),
            ))
            .unwrap();
        if request_id == 5 {
            let snapshot = Snapshot::decode_payload(&response.payload).unwrap();
            assert_eq!(snapshot.commit.as_deref(), Some("mo"));
            assert!(snapshot.composition.is_empty());
        }
    }

    let closed = connection
        .handle(frame(MessageKind::CloseSession, 11, token, 6, Vec::new()))
        .unwrap();
    assert_eq!(closed.header.kind, MessageKind::CloseSessionAck);

    let event = KeyEvent {
        virtual_key: 0x41,
        scan_code: 0,
        modifiers: 0,
        key_down: true,
        repeat: false,
    };
    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            11,
            token,
            7,
            event.encode_payload().unwrap(),
        ))
        .unwrap();
    assert_eq!(error_code(response), ERROR_NO_SUCH_SESSION);
    assert!(
        connection
            .engine()
            .backend()
            .events()
            .iter()
            .any(|event| matches!(event, FakeEvent::SessionDestroyed { backend_session: 1 }))
    );
}

#[test]
fn loopback_spike_round_trips_real_framed_tcp_io() {
    let listener =
        mo_broker::tcp_loopback_spike::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into())
            .unwrap();
    let address = listener.local_addr().unwrap();
    let server =
        thread::spawn(move || mo_broker::tcp_loopback_spike::serve_listener(listener, true));

    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    let hello_payload = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: FEATURE_KEY_EVENTS,
        max_payload_len: u32::try_from(MAX_PAYLOAD_LEN).unwrap(),
    }
    .encode_payload()
    .unwrap();
    mo_ipc::write_frame(
        &mut stream,
        &frame(MessageKind::Hello, 0, 0, 1, hello_payload),
    )
    .unwrap();
    let hello_ack = mo_ipc::read_frame(&mut stream).unwrap();
    assert_eq!(hello_ack.header.kind, MessageKind::HelloAck);
    let generation = hello_ack.header.connection_generation;
    assert_ne!(generation, 0);

    mo_ipc::write_frame(
        &mut stream,
        &frame(MessageKind::OpenSession, generation, 0, 2, Vec::new()),
    )
    .unwrap();
    let opened = mo_ipc::read_frame(&mut stream).unwrap();
    assert_eq!(opened.header.kind, MessageKind::OpenSessionAck);
    assert_ne!(opened.header.session_token, 0);

    drop(stream);
    server.join().unwrap().unwrap();
}

#[cfg(windows)]
#[test]
fn authenticated_named_pipe_round_trips_broker_handshake() {
    use mo_windows_pipe::{PipeAddress, PipeClient, PipeListener};

    let address = PipeAddress::new(&format!("broker-test-{}", std::process::id())).unwrap();
    let listener = PipeListener::bind(address.clone()).unwrap();
    assert!(listener.rejects_remote_clients().unwrap());
    let server = thread::spawn(move || mo_broker::windows_named_pipe::serve_listener(listener));

    let mut stream = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
    let hello_payload = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: FEATURE_KEY_EVENTS,
        max_payload_len: u32::try_from(MAX_PAYLOAD_LEN).unwrap(),
    }
    .encode_payload()
    .unwrap();
    mo_ipc::write_frame(
        &mut stream,
        &frame(MessageKind::Hello, 0, 0, 1, hello_payload),
    )
    .unwrap();

    let hello_ack = mo_ipc::read_frame(&mut stream).unwrap();
    assert_eq!(hello_ack.header.kind, MessageKind::HelloAck);
    assert_ne!(hello_ack.header.connection_generation, 0);

    drop(stream);
    server.join().unwrap().unwrap();
}

#[cfg(windows)]
#[test]
fn named_pipe_broker_rearms_and_preserves_global_engine_order() {
    use mo_engine::FakeBackend;
    use mo_windows_pipe::{PipeAddress, PipeClient, PipeListener};

    fn transact(address: &PipeAddress, virtual_key: u32) -> u64 {
        let mut stream = PipeClient::connect(address, Duration::from_secs(2)).unwrap();
        let hello_payload = Hello {
            supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
            features: FEATURE_KEY_EVENTS,
            max_payload_len: u32::try_from(MAX_PAYLOAD_LEN).unwrap(),
        }
        .encode_payload()
        .unwrap();
        mo_ipc::write_frame(
            &mut stream,
            &frame(MessageKind::Hello, 0, 0, 1, hello_payload),
        )
        .unwrap();
        let hello_ack = mo_ipc::read_frame(&mut stream).unwrap();
        let generation = hello_ack.header.connection_generation;

        mo_ipc::write_frame(
            &mut stream,
            &frame(MessageKind::OpenSession, generation, 0, 2, Vec::new()),
        )
        .unwrap();
        let opened = mo_ipc::read_frame(&mut stream).unwrap();
        let token = opened.header.session_token;

        let key = KeyEvent {
            virtual_key,
            scan_code: 0,
            modifiers: 0,
            key_down: true,
            repeat: false,
        };
        mo_ipc::write_frame(
            &mut stream,
            &frame(
                MessageKind::KeyEvent,
                generation,
                token,
                3,
                key.encode_payload().unwrap(),
            ),
        )
        .unwrap();
        let snapshot_frame = mo_ipc::read_frame(&mut stream).unwrap();
        let snapshot = Snapshot::decode_payload(&snapshot_frame.payload).unwrap();

        mo_ipc::write_frame(
            &mut stream,
            &frame(MessageKind::CloseSession, generation, token, 4, Vec::new()),
        )
        .unwrap();
        let closed = mo_ipc::read_frame(&mut stream).unwrap();
        assert_eq!(closed.header.kind, MessageKind::CloseSessionAck);
        snapshot.revision
    }

    let address = PipeAddress::new(&format!("broker-rearm-test-{}", std::process::id())).unwrap();
    let listener = PipeListener::bind(address.clone()).unwrap();
    let server = thread::spawn(move || {
        mo_broker::windows_named_pipe::serve_listener_loop_with_backend_factory(
            listener,
            || Ok::<_, std::io::Error>(FakeBackend::new()),
            Some(2),
        )
    });

    let first_revision = transact(&address, u32::from(b'A'));
    let second_revision = transact(&address, u32::from(b'B'));
    assert!(second_revision > first_revision);
    server.join().unwrap().unwrap();
}
fn candidate_hello(connection: &mut BrokerConnection) {
    let payload = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: FEATURE_KEY_EVENTS | mo_ipc::FEATURE_CANDIDATE_ACTIONS,
        max_payload_len: MAX_PAYLOAD_LEN as u32,
    }
    .encode_payload()
    .unwrap();
    let ack = connection
        .handle(frame(MessageKind::Hello, 0, 0, 1, payload))
        .unwrap();
    assert_eq!(HelloAck::decode_payload(&ack.payload).unwrap().features, 3);
}

fn candidate_key(
    connection: &mut BrokerConnection,
    token: u64,
    id: u64,
    key: u32,
    down: bool,
) -> Snapshot {
    let payload = KeyEvent {
        virtual_key: key,
        scan_code: 0,
        modifiers: 0,
        key_down: down,
        repeat: false,
    }
    .encode_payload()
    .unwrap();
    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            connection.connection_generation(),
            token,
            id,
            payload,
        ))
        .unwrap();
    Snapshot::decode_payload(&response.payload).unwrap()
}

fn candidate_request(
    connection: &mut BrokerConnection,
    token: u64,
    id: u64,
    revision: u64,
    action: mo_ipc::CandidateActionKind,
    index: u32,
) -> Frame {
    let payload = mo_ipc::CandidateAction {
        expected_revision: revision,
        action,
        index,
    }
    .encode_payload()
    .unwrap();
    connection
        .handle(frame(
            MessageKind::CandidateAction,
            connection.connection_generation(),
            token,
            id,
            payload,
        ))
        .unwrap()
}

#[test]
fn candidate_actions_require_negotiation_and_a_current_session_page() {
    use mo_ipc::CandidateActionKind::Select;
    let mut old = BrokerConnection::new(80);
    hello(&mut old, 1);
    let token = open(&mut old, 2);
    let page = candidate_key(&mut old, token, 3, 0x4d, true);
    let events = old.engine().backend().events().len();
    let response = candidate_request(&mut old, token, 4, page.revision, Select, 0);
    assert_eq!(error_code(response), mo_broker::ERROR_BAD_REQUEST);
    assert_eq!(old.engine().backend().events().len(), events);

    let mut connection = BrokerConnection::new(81);
    candidate_hello(&mut connection);
    let token = open(&mut connection, 2);
    let response = candidate_request(&mut connection, token, 3, 1, Select, 0);
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
    let page = candidate_key(&mut connection, token, 4, 0x4d, true);
    let other = open(&mut connection, 5);
    let events = connection.engine().backend().events().len();
    let response = candidate_request(&mut connection, other, 6, page.revision, Select, 0);
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
    let response = candidate_request(&mut connection, 999, 7, page.revision, Select, 0);
    assert_eq!(error_code(response), ERROR_NO_SUCH_SESSION);
    let response = candidate_request(&mut connection, token, 8, page.revision, Select, 2);
    assert_eq!(error_code(response), mo_broker::ERROR_BAD_REQUEST);
    assert_eq!(connection.engine().backend().events().len(), events);
}

#[test]
fn candidate_actions_page_select_and_reject_replays_before_backend_dispatch() {
    use mo_ipc::CandidateActionKind::{NextPage, PreviousPage, Select};
    let mut connection = BrokerConnection::new(82);
    candidate_hello(&mut connection);
    let token = open(&mut connection, 2);
    let first = candidate_key(&mut connection, token, 3, 0x4d, true);
    let response = candidate_request(&mut connection, token, 4, first.revision, NextPage, 0);
    let next = Snapshot::decode_payload(&response.payload).unwrap();
    assert_eq!(next.candidates, vec!["m#2", "M#2"]);
    assert!(next.commit.is_none());
    let events = connection.engine().backend().events().len();
    let response = candidate_request(&mut connection, token, 5, first.revision, Select, 1);
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
    assert_eq!(connection.engine().backend().events().len(), events);
    let response = candidate_request(&mut connection, token, 6, next.revision, PreviousPage, 0);
    let previous = Snapshot::decode_payload(&response.payload).unwrap();
    assert_eq!(previous.candidates, first.candidates);
    let response = candidate_request(&mut connection, token, 7, previous.revision, Select, 1);
    let selected = Snapshot::decode_payload(&response.payload).unwrap();
    assert_eq!(selected.commit.as_deref(), Some("M"));
    assert!(selected.composition.is_empty());
    let events = connection.engine().backend().events().len();
    let response = candidate_request(&mut connection, token, 8, selected.revision, Select, 0);
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
    assert_eq!(connection.engine().backend().events().len(), events);
}

#[test]
fn even_unhandled_key_up_invalidates_the_previous_candidate_revision() {
    use mo_ipc::CandidateActionKind::Select;
    let mut connection = BrokerConnection::new(83);
    candidate_hello(&mut connection);
    let token = open(&mut connection, 2);
    let first = candidate_key(&mut connection, token, 3, 0x4d, true);
    let release = candidate_key(&mut connection, token, 4, 0x4d, false);
    assert!(!release.handled);
    assert_eq!(release.candidates, first.candidates);
    let response = candidate_request(&mut connection, token, 5, first.revision, Select, 1);
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
    let response = candidate_request(&mut connection, token, 6, release.revision, Select, 1);
    assert_eq!(
        Snapshot::decode_payload(&response.payload)
            .unwrap()
            .commit
            .as_deref(),
        Some("M")
    );
}
#[derive(Default)]
struct FailCandidateBackend(mo_engine::FakeBackend);

impl EngineBackend for FailCandidateBackend {
    type Session = <mo_engine::FakeBackend as EngineBackend>::Session;
    type Error = &'static str;
    fn create_session(&mut self, options: SessionOptions) -> Result<Self::Session, Self::Error> {
        self.0.create_session(options).map_err(|_| "create")
    }
    fn apply(
        &mut self,
        session: &mut Self::Session,
        command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error> {
        if matches!(command, EngineCommand::SelectCandidate { .. }) {
            return Err("candidate failure");
        }
        self.0.apply(session, command).map_err(|_| "apply")
    }
}

#[test]
fn a_backend_failure_revokes_the_previous_candidate_page() {
    use mo_ipc::{CandidateAction, CandidateActionKind, FEATURE_CANDIDATE_ACTIONS};
    let mut connection = BrokerConnection::with_backend(84, FailCandidateBackend::default());
    let payload = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: FEATURE_KEY_EVENTS | FEATURE_CANDIDATE_ACTIONS,
        max_payload_len: MAX_PAYLOAD_LEN as u32,
    }
    .encode_payload()
    .unwrap();
    connection
        .handle(frame(MessageKind::Hello, 0, 0, 1, payload))
        .unwrap();
    let token = open(&mut connection, 2);
    let key = KeyEvent {
        virtual_key: 0x4d,
        scan_code: 0,
        modifiers: 0,
        key_down: true,
        repeat: false,
    };
    let response = connection
        .handle(frame(
            MessageKind::KeyEvent,
            84,
            token,
            3,
            key.encode_payload().unwrap(),
        ))
        .unwrap();
    let page = Snapshot::decode_payload(&response.payload).unwrap();
    let payload = CandidateAction {
        expected_revision: page.revision,
        action: CandidateActionKind::Select,
        index: 0,
    }
    .encode_payload()
    .unwrap();
    let response = connection
        .handle(frame(
            MessageKind::CandidateAction,
            84,
            token,
            4,
            payload.clone(),
        ))
        .unwrap();
    assert_eq!(error_code(response), ERROR_ENGINE_FAILURE);
    let response = connection
        .handle(frame(MessageKind::CandidateAction, 84, token, 5, payload))
        .unwrap();
    assert_eq!(error_code(response), mo_broker::ERROR_STALE_CANDIDATES);
}
