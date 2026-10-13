use mo_ipc::{
    CompositionPositions, CompositionSnapshot, DetailedSnapshot, PayloadCodec, PresentationSnapshot,
};
fn snapshot(text: &str, p: Option<CompositionPositions>) -> CompositionSnapshot {
    CompositionSnapshot {
        positions: p,
        presentation: PresentationSnapshot {
            page: None,
            snapshot: DetailedSnapshot {
                revision: 1,
                handled: true,
                composition: text.into(),
                commit: None,
                candidates: vec![],
            },
        },
    }
}
fn positions(cursor: u32, start: u32, end: u32) -> Option<CompositionPositions> {
    Some(CompositionPositions {
        cursor_utf8_bytes: cursor,
        selection_start_utf8_bytes: start,
        selection_end_utf8_bytes: end,
    })
}
#[test]
fn utf8_positions_and_legacy_body_have_independent_golden_bytes() {
    let value = snapshot("a😀墨", positions(5, 1, 8));
    let bytes = value.encode_payload().unwrap();
    assert_eq!(&bytes[..13], &[1, 5, 0, 0, 0, 1, 0, 0, 0, 8, 0, 0, 0]);
    assert_eq!(&bytes[13..], value.presentation.encode_payload().unwrap());
    assert_eq!(CompositionSnapshot::decode_payload(&bytes).unwrap(), value);
}
#[test]
fn malformed_positions_cannot_cross_utf8_boundaries_or_text_limits() {
    for p in [
        positions(2, 0, 1),
        positions(9, 0, 1),
        positions(1, 8, 1),
        None,
    ] {
        assert!(snapshot("a😀墨", p).encode_payload().is_err());
    }
    assert!(snapshot("", positions(0, 0, 0)).encode_payload().is_err());
    let canonical = snapshot("", None).encode_payload().unwrap();
    for n in 0..13 {
        assert!(CompositionSnapshot::decode_payload(&canonical[..n]).is_err());
    }
    let mut invalid = canonical.clone();
    invalid[0] = 2;
    assert!(CompositionSnapshot::decode_payload(&invalid).is_err());
    let mut invalid = canonical;
    invalid[1] = 1;
    assert!(CompositionSnapshot::decode_payload(&invalid).is_err());
    let mut valid = snapshot("a😀墨", positions(5, 1, 8))
        .encode_payload()
        .unwrap();
    valid[1] = 2;
    assert!(CompositionSnapshot::decode_payload(&valid).is_err());
}
