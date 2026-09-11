use mo_broker::{BrokerConnection, ERROR_BAD_HANDSHAKE, ERROR_NO_SUCH_SESSION, ERROR_OUT_OF_ORDER};
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

fn hello(connection: &mut BrokerConnection, request_id: u64) -> Frame {
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

fn open(connection: &mut BrokerConnection, request_id: u64) -> u64 {
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
fn diagnostic_ascii_echo_keeps_sessions_isolated() {
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
    assert_eq!(snapshot.composition, "m");
    assert_eq!(snapshot.candidates, ["m"]);

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
    assert_eq!(snapshot.composition, "m");
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
