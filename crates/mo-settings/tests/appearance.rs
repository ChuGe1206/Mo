use mo_settings::{Appearance, CandidateSkin, Settings, Theme};
#[test]
fn appearance_is_optional_for_old_documents_and_complete_when_present() {
    let old = Settings::default().encode().unwrap();
    assert!(!old.contains("skin_"));
    assert_eq!(
        Settings::decode(old.as_bytes()).unwrap().appearance,
        Appearance::default()
    );
    for skin in 0..=6 {
        let settings = Settings {
            appearance: Appearance {
                skin: CandidateSkin::from_index(skin).unwrap(),
                window_theme: Theme::Dark,
                vertical: true,
                font_size: 24,
                radius: 20,
                ..Appearance::default()
            },
            ..Settings::default()
        };
        let encoded = settings.encode().unwrap();
        assert_eq!(Settings::decode(encoded.as_bytes()).unwrap(), settings);
        let missing = encoded
            .lines()
            .filter(|line| !line.starts_with("skin_text="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(Settings::decode(missing.as_bytes()).is_err());
        assert!(Settings::decode(format!("{encoded}skin_text=000000\n").as_bytes()).is_err());
    }
}
#[test]
fn imported_skin_accepts_only_bounded_data_and_preserves_unicode_free_contract() {
    let original = Appearance {
        skin: CandidateSkin::Custom,
        vertical: true,
        ..Appearance::default()
    };
    let encoded = original.encode_skin().unwrap();
    assert_eq!(
        Appearance::decode_skin(encoded.as_bytes()).unwrap(),
        original
    );
    for bad in [
        encoded.replace("FFFFFF", "FFFFFFFF"),
        encoded.replace("radius=12", "radius=21"),
        encoded.replace("font_size=16", "font_size=25"),
        encoded.replace("vertical=true", "vertical=1"),
        format!("{encoded}script=lua\n"),
        format!("{encoded}accent=123456\n"),
        "x".repeat(1025),
    ] {
        assert!(Appearance::decode_skin(bad.as_bytes()).is_err());
    }
}
