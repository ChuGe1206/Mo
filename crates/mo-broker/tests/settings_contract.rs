//! Acceptance contracts across independently owned wire and persisted formats.
#![forbid(unsafe_code)]

use mo_ipc::{PayloadCodec, PresentationSettings, SettingsSnapshot};
use mo_settings::{Appearance, CandidateSkin, Settings};

fn projection() -> PresentationSettings {
    let settings = Settings::default();
    let appearance = settings.appearance;
    PresentationSettings {
        settings: SettingsSnapshot {
            revision: 1,
            origin: mo_ipc::SettingsOrigin::Defaults,
            input_scheme: mo_ipc::InputScheme::FullPinyin,
            character_set: mo_ipc::CharacterSet::Simplified,
            candidate_page_size: settings.candidate_page_size,
            theme: mo_ipc::Theme::System,
            show_comments: settings.show_comments,
            emoji: settings.emoji,
            local_learning: settings.local_learning,
            privacy_mode: settings.privacy_mode,
            effective_learning: settings.effective_learning(),
        },
        skin: appearance.skin as u8,
        background: appearance.background,
        accent: appearance.accent,
        text: appearance.text,
        radius: appearance.radius,
        font_size: appearance.font_size,
        vertical: appearance.vertical,
    }
}

#[test]
fn all_page_size_values_have_matching_settings_and_wire_acceptance() {
    for candidate_page_size in u8::MIN..=u8::MAX {
        let settings = Settings {
            candidate_page_size,
            ..Settings::default()
        };
        let mut wire = projection();
        wire.settings.candidate_page_size = candidate_page_size;
        assert_eq!(
            settings.validate().is_ok(),
            wire.encode_payload().is_ok(),
            "candidate page size {candidate_page_size}"
        );
    }
}

#[test]
fn all_radius_and_font_values_have_matching_persistence_and_wire_acceptance() {
    for value in u8::MIN..=u8::MAX {
        let appearance = Appearance {
            radius: value,
            ..Appearance::default()
        };
        let mut wire = projection();
        wire.radius = value;
        assert_eq!(
            appearance.validate().is_ok(),
            wire.encode_payload().is_ok(),
            "candidate radius {value}"
        );
        let appearance = Appearance {
            font_size: value,
            ..Appearance::default()
        };
        wire = projection();
        wire.font_size = value;
        assert_eq!(
            appearance.validate().is_ok(),
            wire.encode_payload().is_ok(),
            "candidate font size {value}"
        );
    }
}

#[test]
fn skin_ids_and_rgb_boundaries_have_matching_persistence_and_wire_acceptance() {
    for skin in u8::MIN..=u8::MAX {
        let mut wire = projection();
        wire.skin = skin;
        assert_eq!(
            CandidateSkin::from_index(skin).is_some(),
            wire.encode_payload().is_ok(),
            "candidate skin {skin}"
        );
    }
    for color in [0, 0x00ff_ffff, 0x0100_0000, u32::MAX] {
        for component in 0..3 {
            let mut appearance = Appearance::default();
            let mut wire = projection();
            match component {
                0 => {
                    appearance.background = color;
                    wire.background = color;
                }
                1 => {
                    appearance.accent = color;
                    wire.accent = color;
                }
                _ => {
                    appearance.text = color;
                    wire.text = color;
                }
            }
            assert_eq!(
                appearance.validate().is_ok(),
                wire.encode_payload().is_ok(),
                "RGB component {component}, value {color:#x}"
            );
        }
    }
}
