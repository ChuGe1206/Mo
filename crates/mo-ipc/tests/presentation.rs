use mo_ipc::{
    CandidatePage, DetailedCandidate, DetailedSnapshot, PayloadCodec, PresentationSnapshot,
    SymbolAction,
};

fn sample() -> PresentationSnapshot {
    PresentationSnapshot {
        snapshot: DetailedSnapshot {
            revision: 1,
            handled: true,
            composition: "a".into(),
            commit: None,
            candidates: vec![DetailedCandidate {
                text: "甲".into(),
                comment: None,
                label: Some("1".into()),
            }],
        },
        page: Some(CandidatePage {
            number: 0,
            has_previous: false,
            has_next: true,
            highlighted: Some(0),
        }),
    }
}
#[test]
fn presentation_is_bounded_and_leaves_detailed_body_unchanged() {
    let sample = sample();
    let bytes = sample.encode_payload().unwrap();
    assert_eq!(&bytes[8..], sample.snapshot.encode_payload().unwrap());
    assert_eq!(
        PresentationSnapshot::decode_payload(&bytes).unwrap(),
        sample
    );
    for length in 0..bytes.len() {
        assert!(PresentationSnapshot::decode_payload(&bytes[..length]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(PresentationSnapshot::decode_payload(&trailing).is_err());
    let mut bad_highlight = bytes.clone();
    bad_highlight[7] = 1;
    assert!(PresentationSnapshot::decode_payload(&bad_highlight).is_err());
    let mut bad_previous = bytes.clone();
    bad_previous[5] = 1;
    assert!(PresentationSnapshot::decode_payload(&bad_previous).is_err());
    let mut absent = sample.clone();
    absent.page = None;
    let mut bytes = absent.encode_payload().unwrap();
    bytes[6] = 1;
    assert!(PresentationSnapshot::decode_payload(&bytes).is_err());
}
#[test]
fn symbol_action_rejects_zero_revision_truncation_and_trailing_data() {
    let action = SymbolAction {
        expected_revision: 7,
        index: 20,
    };
    let bytes = action.encode_payload().unwrap();
    assert_eq!(bytes.len(), 12);
    assert_eq!(SymbolAction::decode_payload(&bytes).unwrap(), action);
    for length in 0..12 {
        assert!(SymbolAction::decode_payload(&bytes[..length]).is_err());
    }
    let mut bad = bytes;
    bad.push(0);
    assert!(SymbolAction::decode_payload(&bad).is_err());
    assert!(
        SymbolAction {
            expected_revision: 0,
            index: 0
        }
        .encode_payload()
        .is_err()
    );
}

#[test]
fn appearance_extension_keeps_legacy_prefix_and_rejects_invalid_bytes() {
    use mo_ipc::{
        CharacterSet, InputScheme, PresentationSettings, SettingsOrigin, SettingsSnapshot, Theme,
    };
    let snapshot = PresentationSettings {
        settings: SettingsSnapshot {
            revision: 1,
            origin: SettingsOrigin::Defaults,
            input_scheme: InputScheme::FullPinyin,
            character_set: CharacterSet::Simplified,
            candidate_page_size: 5,
            theme: Theme::System,
            show_comments: true,
            emoji: true,
            local_learning: true,
            privacy_mode: false,
            effective_learning: true,
        },
        skin: 6,
        background: 0xffffff,
        accent: 0x176d5e,
        text: 0x25352f,
        radius: 12,
        font_size: 16,
        vertical: true,
    };
    let bytes = snapshot.encode_payload().unwrap();
    assert_eq!(bytes.len(), 34);
    assert_eq!(&bytes[..18], snapshot.settings.encode_payload().unwrap());
    assert_eq!(
        PresentationSettings::decode_payload(&bytes).unwrap(),
        snapshot
    );
    for length in 0..bytes.len() {
        assert!(PresentationSettings::decode_payload(&bytes[..length]).is_err());
    }
    for (offset, value) in [(18, 7), (22, 1), (31, 21), (32, 25), (33, 2)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(PresentationSettings::decode_payload(&bad).is_err());
    }
}
