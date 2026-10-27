use mo_ipc::{
    CompositionSnapshot, DetailedSnapshot, InputModeAction, InputModeRequest, ModeSnapshot,
    PayloadCodec, PresentationSnapshot,
};

fn empty() -> CompositionSnapshot {
    CompositionSnapshot {
        positions: None,
        presentation: PresentationSnapshot {
            page: None,
            snapshot: DetailedSnapshot {
                revision: 1,
                handled: false,
                composition: String::new(),
                commit: None,
                candidates: Vec::new(),
            },
        },
    }
}

#[test]
fn mode_header_is_canonical_and_legacy_snapshot_bytes_are_unchanged() {
    let legacy = empty();
    let legacy_bytes = legacy.encode_payload().unwrap();
    for ascii_mode in [false, true] {
        for disabled in [false, true] {
            let snapshot = ModeSnapshot {
                ascii_mode,
                disabled,
                composition: legacy.clone(),
            };
            let encoded = snapshot.encode_payload().unwrap();
            assert_eq!(&encoded[..2], &[u8::from(ascii_mode), u8::from(disabled)]);
            assert_eq!(&encoded[2..], legacy_bytes);
            assert_eq!(ModeSnapshot::decode_payload(&encoded).unwrap(), snapshot);
            for length in 0..encoded.len() {
                assert!(ModeSnapshot::decode_payload(&encoded[..length]).is_err());
            }
            let mut extra = encoded.clone();
            extra.push(0);
            assert!(ModeSnapshot::decode_payload(&extra).is_err());
            for offset in [0, 1] {
                let mut bad = encoded.clone();
                bad[offset] = 2;
                assert!(ModeSnapshot::decode_payload(&bad).is_err());
            }
        }
    }
}

#[test]
fn input_mode_requests_reject_unknown_actions_revisions_and_trailing_bytes() {
    for (revision, action, golden) in [
        (0, InputModeAction::Inspect, [0, 0, 0, 0, 0, 0, 0, 0, 0]),
        (17, InputModeAction::Chinese, [17, 0, 0, 0, 0, 0, 0, 0, 1]),
        (18, InputModeAction::English, [18, 0, 0, 0, 0, 0, 0, 0, 2]),
    ] {
        let request = InputModeRequest {
            expected_revision: revision,
            action,
        };
        assert_eq!(request.encode_payload().unwrap(), golden);
        assert_eq!(InputModeRequest::decode_payload(&golden).unwrap(), request);
        for length in 0..golden.len() {
            assert!(InputModeRequest::decode_payload(&golden[..length]).is_err());
        }
        let mut extra = golden.to_vec();
        extra.push(0);
        assert!(InputModeRequest::decode_payload(&extra).is_err());
    }
    for bad in [
        [0, 0, 0, 0, 0, 0, 0, 0, 1],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 3],
    ] {
        assert!(InputModeRequest::decode_payload(&bad).is_err());
    }
    assert!(
        InputModeRequest {
            expected_revision: 0,
            action: InputModeAction::English
        }
        .encode_payload()
        .is_err()
    );
}
