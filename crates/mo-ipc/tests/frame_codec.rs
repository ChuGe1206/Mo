use std::io::Cursor;

use mo_ipc::{
    CURRENT_VERSION, CodecError, FLAG_RESPONSE, Frame, FrameError, Hello, MAX_CANDIDATES,
    MAX_PAYLOAD_LEN, MIN_NEGOTIATED_PAYLOAD_LEN, MessageKind, PayloadCodec, ProtocolVersion,
    Snapshot, VersionRange, negotiate_version, read_frame, write_frame,
};

#[test]
fn frame_round_trip_preserves_metadata_and_payload() {
    let frame = Frame::new(
        CURRENT_VERSION,
        MessageKind::KeyEvent,
        0,
        41,
        9,
        123,
        b"bounded payload".to_vec(),
    )
    .unwrap();

    let encoded = frame.encode().unwrap();
    assert_eq!(Frame::decode(&encoded).unwrap(), frame);
}

#[test]
fn stream_codec_reads_exactly_one_frame_at_a_time() {
    let first = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 7, 0, 1, vec![]).unwrap();
    let second = Frame::new(
        CURRENT_VERSION,
        MessageKind::Pong,
        FLAG_RESPONSE,
        7,
        0,
        2,
        vec![],
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &first).unwrap();
    write_frame(&mut bytes, &second).unwrap();

    let mut cursor = Cursor::new(bytes);
    assert_eq!(read_frame(&mut cursor).unwrap(), first);
    assert_eq!(read_frame(&mut cursor).unwrap(), second);
}

#[test]
fn decoder_rejects_oversized_payload_before_reading_it() {
    let valid = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 1, 0, 1, vec![]).unwrap();
    let mut encoded = valid.encode().unwrap();
    let oversized = u32::try_from(MAX_PAYLOAD_LEN + 1).unwrap();
    encoded[16..20].copy_from_slice(&oversized.to_le_bytes());
    encoded.truncate(mo_ipc::HEADER_LEN);

    assert!(matches!(
        Frame::decode(&encoded),
        Err(FrameError::PayloadTooLarge { .. })
    ));
}

#[test]
fn decoder_rejects_checksum_mismatch() {
    let frame = Frame::new(
        CURRENT_VERSION,
        MessageKind::KeyEvent,
        0,
        1,
        2,
        3,
        b"abc".to_vec(),
    )
    .unwrap();
    let mut encoded = frame.encode().unwrap();
    *encoded.last_mut().unwrap() ^= 0xff;

    assert!(matches!(
        Frame::decode(&encoded),
        Err(FrameError::ChecksumMismatch { .. })
    ));
}

#[test]
fn decoder_rejects_truncation_and_trailing_data() {
    let frame = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 1, 0, 1, vec![]).unwrap();
    let encoded = frame.encode().unwrap();
    assert!(matches!(
        Frame::decode(&encoded[..encoded.len() - 1]),
        Err(FrameError::Truncated { .. })
    ));

    let mut trailing = encoded;
    trailing.push(0);
    assert!(matches!(
        Frame::decode(&trailing),
        Err(FrameError::TrailingBytes(1))
    ));
}

#[test]
fn snapshot_round_trip_is_unicode_safe() {
    let snapshot = Snapshot {
        revision: 8,
        handled: true,
        composition: "ni'hao 👋".to_owned(),
        commit: Some("你好".to_owned()),
        candidates: vec!["你好".to_owned(), "拟好".to_owned()],
    };
    let encoded = snapshot.encode_payload().unwrap();
    assert_eq!(Snapshot::decode_payload(&encoded).unwrap(), snapshot);
}

#[test]
fn snapshot_codec_enforces_candidate_count() {
    let snapshot = Snapshot {
        revision: 1,
        handled: true,
        composition: String::new(),
        commit: None,
        candidates: vec![String::new(); MAX_CANDIDATES + 1],
    };
    assert!(matches!(
        snapshot.encode_payload(),
        Err(CodecError::TooManyItems { .. })
    ));
}

#[test]
fn payload_codec_rejects_invalid_utf8() {
    // revision, handled, string length, invalid UTF-8, no commit, zero candidates
    let mut payload = vec![0; 8];
    payload.push(1);
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.push(0xff);
    payload.push(0);
    payload.extend_from_slice(&0_u16.to_le_bytes());
    assert!(matches!(
        Snapshot::decode_payload(&payload),
        Err(CodecError::InvalidUtf8(_))
    ));
}

#[test]
fn version_negotiation_selects_highest_shared_minor() {
    let local = VersionRange::new(
        ProtocolVersion { major: 1, minor: 0 },
        ProtocolVersion { major: 1, minor: 3 },
    )
    .unwrap();
    let peer = VersionRange::new(
        ProtocolVersion { major: 1, minor: 1 },
        ProtocolVersion { major: 1, minor: 2 },
    )
    .unwrap();
    assert_eq!(
        negotiate_version(local, peer),
        Some(ProtocolVersion { major: 1, minor: 2 })
    );

    let incompatible = VersionRange::new(
        ProtocolVersion { major: 2, minor: 0 },
        ProtocolVersion { major: 2, minor: 0 },
    )
    .unwrap();
    assert_eq!(negotiate_version(local, incompatible), None);
}

#[test]
fn hello_rejects_claim_above_hard_payload_limit() {
    let hello = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: 0,
        max_payload_len: u32::try_from(MAX_PAYLOAD_LEN + 1).unwrap(),
    };
    assert!(matches!(
        hello.encode_payload(),
        Err(CodecError::InvalidValue(_))
    ));
}

#[test]
fn hello_rejects_peer_limit_too_small_for_a_valid_snapshot() {
    let hello = Hello {
        supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
        features: 0,
        max_payload_len: u32::try_from(MIN_NEGOTIATED_PAYLOAD_LEN - 1).unwrap(),
    };
    assert!(matches!(
        hello.encode_payload(),
        Err(CodecError::InvalidValue(_))
    ));
}
#[test]
fn candidate_actions_are_fixed_bounded_and_canonical() {
    use mo_ipc::{CandidateAction, CandidateActionKind};
    for action in [
        CandidateActionKind::Select,
        CandidateActionKind::PreviousPage,
        CandidateActionKind::NextPage,
    ] {
        let value = CandidateAction {
            expected_revision: 7,
            action,
            index: 0,
        };
        let payload = value.encode_payload().unwrap();
        assert_eq!(payload.len(), 13);
        assert_eq!(CandidateAction::decode_payload(&payload).unwrap(), value);
        for length in 0..13 {
            assert!(CandidateAction::decode_payload(&payload[..length]).is_err());
        }
        let mut extra = payload.clone();
        extra.push(0);
        assert!(CandidateAction::decode_payload(&extra).is_err());
    }
    let value = CandidateAction {
        expected_revision: 7,
        action: CandidateActionKind::Select,
        index: 31,
    };
    let mut payload = value.encode_payload().unwrap();
    payload[8] = 3;
    assert!(CandidateAction::decode_payload(&payload).is_err());
    payload[8] = 1;
    assert!(CandidateAction::decode_payload(&payload).is_err());
    payload[8] = 0;
    payload[9..13].copy_from_slice(&32u32.to_le_bytes());
    assert!(CandidateAction::decode_payload(&payload).is_err());
    payload[9..13].copy_from_slice(&0u32.to_le_bytes());
    payload[..8].fill(0);
    assert!(CandidateAction::decode_payload(&payload).is_err());
}

#[test]
fn settings_snapshot_is_fixed_bounded_and_policy_consistent() {
    use mo_ipc::{CharacterSet, InputScheme, SettingsOrigin, SettingsSnapshot, Theme};

    let value = SettingsSnapshot {
        revision: 9,
        origin: SettingsOrigin::Stored,
        input_scheme: InputScheme::DoublePinyinFlypy,
        character_set: CharacterSet::Traditional,
        candidate_page_size: 7,
        theme: Theme::Dark,
        show_comments: false,
        emoji: true,
        local_learning: true,
        privacy_mode: true,
        effective_learning: false,
    };
    let payload = value.encode_payload().unwrap();
    assert_eq!(payload.len(), 18);
    assert_eq!(SettingsSnapshot::decode_payload(&payload).unwrap(), value);
    for length in 0..payload.len() {
        assert!(SettingsSnapshot::decode_payload(&payload[..length]).is_err());
    }
    let mut extra = payload.clone();
    extra.push(0);
    assert!(SettingsSnapshot::decode_payload(&extra).is_err());

    let mut invalid = payload;
    invalid[0..8].fill(0);
    assert!(SettingsSnapshot::decode_payload(&invalid).is_err());
    invalid = value.encode_payload().unwrap();
    invalid[11] = 10;
    assert!(SettingsSnapshot::decode_payload(&invalid).is_err());
    invalid = value.encode_payload().unwrap();
    invalid[17] = 1;
    assert!(SettingsSnapshot::decode_payload(&invalid).is_err());
}
