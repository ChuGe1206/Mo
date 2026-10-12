#![forbid(unsafe_code)]
use mo_settings_app::text_catalog::{
    MAX_CATALOG_BYTES, MAX_TEXT_BYTES, TextError, TextKey, UiLocale, UiTextCatalog,
};
fn overlay(locale: UiLocale, fields: &str) -> Vec<u8> {
    format!("mo-ui-text\nformat=1\nlocale={}\n{fields}", locale.as_str()).into_bytes()
}
#[test]
fn both_locales_cover_every_key() {
    for locale in [UiLocale::ZhCn, UiLocale::EnUs] {
        let catalog = UiTextCatalog::bundled(locale).unwrap();
        for key in TextKey::ALL {
            assert!(!catalog.get(*key).is_empty(), "{}", key.as_str());
        }
        assert_eq!(catalog.locale(), locale);
    }
    assert_eq!(
        UiTextCatalog::bundled(UiLocale::ZhCn)
            .unwrap()
            .get(TextKey::Comments)
            .as_ref(),
        "候选注释"
    );
    assert_eq!(
        UiTextCatalog::bundled(UiLocale::EnUs)
            .unwrap()
            .get(TextKey::Comments)
            .as_ref(),
        "Candidate comments"
    );
}
#[test]
fn partial_override_preserves_defaults_and_original() {
    let original = UiTextCatalog::bundled(UiLocale::ZhCn).unwrap();
    let updated = original
        .with_override(&overlay(UiLocale::ZhCn, "comments=辅助说明\n"))
        .unwrap();
    assert_eq!(updated.get(TextKey::Comments).as_ref(), "辅助说明");
    assert_eq!(updated.get(TextKey::Save), original.get(TextKey::Save));
    assert_eq!(original.get(TextKey::Comments).as_ref(), "候选注释");
}
#[test]
fn invalid_override_rejects_all_values() {
    let original = UiTextCatalog::bundled(UiLocale::ZhCn).unwrap();
    for (fields, expected) in [
        ("comments=First\ncomments=Second\n", TextError::DuplicateKey),
        ("comments=First\nunknown=Second\n", TextError::UnknownKey),
        ("comments=\n", TextError::InvalidValue),
        ("comments=First\0Second\n", TextError::InvalidValue),
        ("comments=First\tSecond\n", TextError::InvalidValue),
        ("comments={error}\n", TextError::InvalidPlaceholder),
        (
            "skin.operation.failed=Failure\n",
            TextError::InvalidPlaceholder,
        ),
        (
            "skin.operation.failed=Failure {other}\n",
            TextError::InvalidPlaceholder,
        ),
        (
            "skin.operation.failed=Failure {error\n",
            TextError::InvalidPlaceholder,
        ),
        (
            "skin.operation.failed=Failure }{error}\n",
            TextError::InvalidPlaceholder,
        ),
    ] {
        assert_eq!(
            original
                .with_override(&overlay(UiLocale::ZhCn, fields))
                .unwrap_err(),
            expected
        );
        assert_eq!(original.get(TextKey::Comments).as_ref(), "候选注释");
    }
    assert_eq!(
        original
            .with_override(&overlay(UiLocale::EnUs, "comments=First\n"))
            .unwrap_err(),
        TextError::UnsupportedLocale
    );
    assert_eq!(
        original.with_override(&[0xff]).unwrap_err(),
        TextError::InvalidUtf8
    );
}
#[test]
fn size_limits_accept_exact_boundary() {
    let catalog = UiTextCatalog::bundled(UiLocale::ZhCn).unwrap();
    let value = "x".repeat(MAX_TEXT_BYTES);
    assert!(
        catalog
            .with_override(&overlay(UiLocale::ZhCn, &format!("comments={value}\n")))
            .is_ok()
    );
    assert_eq!(
        catalog
            .with_override(&overlay(UiLocale::ZhCn, &format!("comments={value}x\n")))
            .unwrap_err(),
        TextError::InvalidValue
    );
    let mut bytes = overlay(UiLocale::ZhCn, "#");
    bytes.resize(MAX_CATALOG_BYTES, b'x');
    assert!(catalog.with_override(&bytes).is_ok());
    bytes.push(b'x');
    assert_eq!(
        catalog.with_override(&bytes).unwrap_err(),
        TextError::TooLarge
    );
}
#[test]
fn fields_can_reorder_without_reparsing_values() {
    let catalog = UiTextCatalog::bundled(UiLocale::EnUs)
        .unwrap()
        .with_override(&overlay(UiLocale::EnUs, "toggle.state={state} / {label}\n"))
        .unwrap();
    assert_eq!(
        catalog
            .render(
                TextKey::ToggleState,
                &[("label", "{state}"), ("state", "On")]
            )
            .unwrap(),
        "On / {state}"
    );
    assert_eq!(
        catalog
            .render(TextKey::ToggleState, &[("state", "On")])
            .unwrap_err(),
        TextError::MissingArgument
    );
    assert_eq!(
        catalog
            .render(
                TextKey::ToggleState,
                &[("state", "On"), ("state", "On"), ("label", "A")]
            )
            .unwrap_err(),
        TextError::MissingArgument
    );
}
#[test]
fn every_template_renders_in_both_locales() {
    for locale in [UiLocale::ZhCn, UiLocale::EnUs] {
        let catalog = UiTextCatalog::bundled(locale).unwrap();
        for key in [
            TextKey::BackgroundColorValue,
            TextKey::AccentColorValue,
            TextKey::TextColorValue,
        ] {
            assert!(
                catalog
                    .render(key, &[("color", "FFFFFF")])
                    .unwrap()
                    .contains("#FFFFFF")
            );
        }
        assert!(
            catalog
                .render(
                    TextKey::SkinOperationFailed,
                    &[("error", "Synthetic failure")]
                )
                .unwrap()
                .contains("Synthetic failure")
        );
        assert!(
            catalog
                .render(TextKey::ToggleState, &[("label", "A"), ("state", "On")])
                .unwrap()
                .contains("On")
        );
        for key in [
            TextKey::GeneralNavigation,
            TextKey::AppearanceNavigation,
            TextKey::PrivacyNavigation,
            TextKey::BackToSkinsNavigation,
        ] {
            assert!(
                catalog
                    .render(key, &[("label", "Shared label")])
                    .unwrap()
                    .contains("Shared label")
            );
        }
    }
}
#[test]
fn locale_identifiers_and_switching_are_closed() {
    assert_eq!("zh-CN".parse::<UiLocale>().unwrap(), UiLocale::ZhCn);
    assert_eq!("en-US".parse::<UiLocale>().unwrap(), UiLocale::EnUs);
    for invalid in ["", "../en-US", "en-US/", "EN-US", "fr-FR"] {
        assert_eq!(
            invalid.parse::<UiLocale>().unwrap_err(),
            TextError::UnsupportedLocale
        );
    }
    assert_eq!(UiLocale::ZhCn.next(), UiLocale::EnUs);
    assert_eq!(UiLocale::EnUs.next(), UiLocale::ZhCn);
}

struct ProfileFixture(std::path::PathBuf);
impl ProfileFixture {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let build = repository.join("build");
        std::fs::create_dir_all(&build).unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = build.join(format!(
            "mo-ui-text-fixture-{stamp}-{}",
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert_eq!(path.parent().unwrap(), build);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for ProfileFixture {
    fn drop(&mut self) {
        // Only the three known synthetic files created in this owned directory.
        // No recursive cleanup or deletion of unrelated files/directories.
        for name in ["ui-text-en-US.mo", "ui-language-v1.mo", "settings-v1.mo"] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}
#[test]
fn profile_overrides_fail_atomically_and_never_modify_engine_settings() {
    let fixture = ProfileFixture::new();
    let settings = fixture.0.join("settings-v1.mo");
    let original = mo_settings::Settings::default().encode().unwrap();
    std::fs::write(&settings, &original).unwrap();
    let missing = UiTextCatalog::from_profile(UiLocale::EnUs, &fixture.0).unwrap();
    assert!(missing.override_error().is_none());
    let path = fixture.0.join("ui-text-en-US.mo");
    std::fs::write(
        &path,
        overlay(
            UiLocale::EnUs,
            "comments=Custom hints\r\nunknown=Second\r\n",
        ),
    )
    .unwrap();
    let invalid = UiTextCatalog::from_profile(UiLocale::EnUs, &fixture.0).unwrap();
    assert_eq!(invalid.override_error(), Some(TextError::UnknownKey));
    assert_eq!(
        invalid.get(TextKey::Comments),
        missing.get(TextKey::Comments)
    );
    std::fs::write(&path, overlay(UiLocale::EnUs, "comments=Custom hints\r\n")).unwrap();
    let customized = UiTextCatalog::from_profile(UiLocale::EnUs, &fixture.0).unwrap();
    assert_eq!(customized.get(TextKey::Comments).as_ref(), "Custom hints");
    assert_eq!(std::fs::read_to_string(settings).unwrap(), original);
}
#[test]
fn startup_preference_handles_bom_and_rejects_wrong_or_oversized_documents() {
    let fixture = ProfileFixture::new();
    assert_eq!(UiLocale::from_profile(&fixture.0).unwrap(), None);
    let path = fixture.0.join("ui-language-v1.mo");
    std::fs::write(
        &path,
        "\u{feff}mo-ui-language\r\nformat=1\r\nlocale=en-US\r\n",
    )
    .unwrap();
    assert_eq!(
        UiLocale::from_profile(&fixture.0).unwrap(),
        Some(UiLocale::EnUs)
    );
    for document in [
        "mo-ui-language\r\nformat=1\r\nlocale=../en-US\r\n",
        "mo-ui-language\r\nformat=1\r\nlocale=en-US\r\nunknown=true\r\n",
    ] {
        std::fs::write(&path, document).unwrap();
        assert!(UiLocale::from_profile(&fixture.0).is_err());
    }
    std::fs::write(&path, vec![b'x'; MAX_CATALOG_BYTES]).unwrap();
    assert_eq!(
        UiLocale::from_profile(&fixture.0).unwrap_err(),
        TextError::TooLarge
    );
}
