//! Windows wire virtual keys to platform-neutral logical symbols.
//!
//! This is the contract owner for the currently supported Chinese/US layout.
//! Unsupported, media and international/dead keys remain NoSymbol; they must
//! never accidentally become ASCII because their VK encoding overlaps it.

use mo_domain::{KeyEvent as DomainKeyEvent, KeyModifiers, KeyState};
use mo_ipc::KeyEvent as WireKeyEvent;

const VK_NUMPAD_FIRST: u32 = 0x60;
const VK_NUMPAD_LAST: u32 = 0x69;
const VK_FUNCTION_FIRST: u32 = 0x70;
const VK_FUNCTION_LAST: u32 = 0x87;
const XK_FUNCTION_FIRST: u32 = 0xffbe;

pub(crate) fn normalize_key_event(event: WireKeyEvent) -> DomainKeyEvent {
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
    // Windows VK_OEM symbols for the supported US-style Chinese keyboard.
    // Records are (virtual-key encoding, unshifted keysym, shifted keysym).
    const OEM_PUNCTUATION: &[(u32, char, char)] = &[
        (0xba, ';', ':'),
        (0xbb, '=', '+'),
        (0xbc, ',', '<'),
        (0xbd, '-', '_'),
        (0xbe, '.', '>'),
        (0xbf, '/', '?'),
        (0xc0, '`', '~'),
        (0xdb, '[', '{'),
        (0xdc, '\\', '|'),
        (0xdd, ']', '}'),
        (0xde, '\'', '"'),
        (0xe2, '\\', '|'), // VK_OEM_102 on the supported US-style layout
    ];
    if let Some((_, plain, shifted)) = OEM_PUNCTUATION
        .iter()
        .find(|(key, _, _)| *key == virtual_key)
    {
        return u32::from(if modifiers.contains(KeyModifiers::SHIFT) {
            *shifted
        } else {
            *plain
        });
    }
    const SHIFTED_DIGITS: &[u8; 10] = b")!@#$%^&*(";
    if (u32::from(b'0')..=u32::from(b'9')).contains(&virtual_key)
        && modifiers.contains(KeyModifiers::SHIFT)
    {
        return u32::from(SHIFTED_DIGITS[(virtual_key - u32::from(b'0')) as usize]);
    }
    if (VK_NUMPAD_FIRST..=VK_NUMPAD_LAST).contains(&virtual_key) {
        return u32::from(b'0') + virtual_key - VK_NUMPAD_FIRST;
    }
    if (VK_FUNCTION_FIRST..=VK_FUNCTION_LAST).contains(&virtual_key) {
        return XK_FUNCTION_FIRST + virtual_key - VK_FUNCTION_FIRST;
    }
    match virtual_key {
        0x20 => 0x20, // XK_space; control semantics, not inferred text
        0x30..=0x39 => virtual_key,
        0x6a => u32::from(b'*'), // VK_MULTIPLY
        0x6b => u32::from(b'+'), // VK_ADD
        0x6c => u32::from(b','), // VK_SEPARATOR (layout-specific behavior not verified)
        0x6d => u32::from(b'-'), // VK_SUBTRACT
        0x6e => u32::from(b'.'), // VK_DECIMAL (supported US-style layout)
        0x6f => u32::from(b'/'), // VK_DIVIDE
        0x03 => 0xff69,          // XK_Cancel
        0x0c => 0xff0b,          // XK_Clear
        0x13 => 0xff13,          // XK_Pause
        0x2f => 0xff6a,          // XK_Help
        0x5d => 0xff67,          // XK_Menu
        0x90 => 0xff7f,          // XK_Num_Lock
        0x91 => 0xff14,          // XK_Scroll_Lock
        0x10 | 0xa0 => 0xffe1,   // XK_Shift_L
        0xa1 => 0xffe2,          // XK_Shift_R
        0x11 | 0xa2 => 0xffe3,   // XK_Control_L
        0xa3 => 0xffe4,          // XK_Control_R
        0x12 | 0xa4 => 0xffe9,   // XK_Alt_L
        0xa5 => 0xffea,          // XK_Alt_R
        0x14 => 0xffe5,          // XK_Caps_Lock
        0x2c => 0xff61,          // XK_Print
        0x2d => 0xff63,          // XK_Insert
        0x5b => 0xffeb,          // XK_Super_L
        0x5c => 0xffec,          // XK_Super_R
        0x08 => 0xff08,          // XK_BackSpace
        0x09 => 0xff09,          // XK_Tab
        0x0d => 0xff0d,          // XK_Return
        0x1b => 0xff1b,          // XK_Escape
        0x21 => 0xff55,          // XK_Page_Up
        0x22 => 0xff56,          // XK_Page_Down
        0x23 => 0xff57,          // XK_End
        0x24 => 0xff50,          // XK_Home
        0x25 => 0xff51,          // XK_Left
        0x26 => 0xff52,          // XK_Up
        0x27 => 0xff53,          // XK_Right
        0x28 => 0xff54,          // XK_Down
        0x2e => 0xffff,          // XK_Delete
        0x41..=0x5a => {
            let uppercase = modifiers.contains(KeyModifiers::SHIFT)
                ^ modifiers.contains(KeyModifiers::CAPS_LOCK);
            if uppercase {
                virtual_key
            } else {
                virtual_key + u32::from(b'a' - b'A')
            }
        }
        // Unsupported VKs must not collide with printable ASCII keysyms.
        _ => 0, // XK_NoSymbol: leave unsupported input to the host.
    }
}

fn infer_ascii_text(keycode: u32, state: KeyState, modifiers: KeyModifiers) -> Option<char> {
    if state != KeyState::Pressed
        || modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return None;
    }

    match keycode {
        0x21..=0x7e => char::from_u32(keycode),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent Windows VK / X11 keysym acceptance data, not generated from
    // the implementation. Text key fixtures include every printable US key.
    const TEXT_KEYS: &[(u32, char, char)] = &[
        (0x30, '0', ')'),
        (0x31, '1', '!'),
        (0x32, '2', '@'),
        (0x33, '3', '#'),
        (0x34, '4', '$'),
        (0x35, '5', '%'),
        (0x36, '6', '^'),
        (0x37, '7', '&'),
        (0x38, '8', '*'),
        (0x39, '9', '('),
        (0xba, ';', ':'),
        (0xbb, '=', '+'),
        (0xbc, ',', '<'),
        (0xbd, '-', '_'),
        (0xbe, '.', '>'),
        (0xbf, '/', '?'),
        (0xc0, '`', '~'),
        (0xdb, '[', '{'),
        (0xdc, '\\', '|'),
        (0xdd, ']', '}'),
        (0xde, '\'', '"'),
        (0xe2, '\\', '|'),
        (0x60, '0', '0'),
        (0x61, '1', '1'),
        (0x62, '2', '2'),
        (0x63, '3', '3'),
        (0x64, '4', '4'),
        (0x65, '5', '5'),
        (0x66, '6', '6'),
        (0x67, '7', '7'),
        (0x68, '8', '8'),
        (0x69, '9', '9'),
        (0x6a, '*', '*'),
        (0x6b, '+', '+'),
        (0x6c, ',', ','),
        (0x6d, '-', '-'),
        (0x6e, '.', '.'),
        (0x6f, '/', '/'),
    ];
    const CONTROL_KEYS: &[(u32, u32)] = &[
        (0x03, 0xff69),
        (0x08, 0xff08),
        (0x09, 0xff09),
        (0x0c, 0xff0b),
        (0x0d, 0xff0d),
        (0x10, 0xffe1),
        (0x11, 0xffe3),
        (0x12, 0xffe9),
        (0x13, 0xff13),
        (0x14, 0xffe5),
        (0x1b, 0xff1b),
        (0x20, 0x20),
        (0x21, 0xff55),
        (0x22, 0xff56),
        (0x23, 0xff57),
        (0x24, 0xff50),
        (0x25, 0xff51),
        (0x26, 0xff52),
        (0x27, 0xff53),
        (0x28, 0xff54),
        (0x2c, 0xff61),
        (0x2d, 0xff63),
        (0x2e, 0xffff),
        (0x2f, 0xff6a),
        (0x5b, 0xffeb),
        (0x5c, 0xffec),
        (0x5d, 0xff67),
        (0x90, 0xff7f),
        (0x91, 0xff14),
        (0xa0, 0xffe1),
        (0xa1, 0xffe2),
        (0xa2, 0xffe3),
        (0xa3, 0xffe4),
        (0xa4, 0xffe9),
        (0xa5, 0xffea),
    ];

    #[test]
    fn entire_vk_byte_space_preserves_text_shortcuts_release_and_repeat() {
        const VK_BYTE_LIMIT: u32 = 256;
        const KNOWN_MODIFIER_COMBINATIONS: u16 = 32;
        const SHIFT_BITS: u16 = 1;
        const CAPS_LOCK_BITS: u16 = 16;
        const COMMAND_MODIFIER_BITS: u16 = 14; // Control | Alt | Super wire bits
        for virtual_key in 0..VK_BYTE_LIMIT {
            for modifiers in 0..KNOWN_MODIFIER_COMBINATIONS {
                let shift = modifiers & SHIFT_BITS != 0;
                let caps = modifiers & CAPS_LOCK_BITS != 0;
                let expected_symbol = if let Some((_, plain, shifted)) =
                    TEXT_KEYS.iter().find(|(vk, _, _)| *vk == virtual_key)
                {
                    u32::from(if shift { *shifted } else { *plain })
                } else if let Ok(byte) = u8::try_from(virtual_key)
                    && byte.is_ascii_uppercase()
                {
                    u32::from(if shift ^ caps {
                        byte
                    } else {
                        byte.to_ascii_lowercase()
                    })
                } else if (0x70..=0x87).contains(&virtual_key) {
                    0xffbe + virtual_key - 0x70
                } else {
                    CONTROL_KEYS
                        .iter()
                        .find(|(vk, _)| *vk == virtual_key)
                        .map_or(0, |(_, symbol)| *symbol)
                };
                for key_down in [false, true] {
                    for repeat in [false, true] {
                        let input = WireKeyEvent {
                            virtual_key,
                            scan_code: 0,
                            modifiers,
                            key_down,
                            repeat,
                        };
                        let result = normalize_key_event(input);
                        assert_eq!(
                            result.keycode, expected_symbol,
                            "VK {virtual_key:#x}, modifiers {modifiers:#x}"
                        );
                        let text = if key_down
                            && modifiers & COMMAND_MODIFIER_BITS == 0
                            && (0x21..=0x7e).contains(&expected_symbol)
                        {
                            char::from_u32(expected_symbol)
                        } else {
                            None
                        };
                        assert_eq!(result.text, text);
                        assert_eq!(result.repeat, repeat);
                        assert_eq!(result.modifiers.bits(), u32::from(modifiers));
                        assert_eq!(
                            result.state,
                            if key_down {
                                KeyState::Pressed
                            } else {
                                KeyState::Released
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn out_of_range_virtual_keys_cannot_become_printable_text() {
        for virtual_key in [256, 0x0100_0041, u32::MAX] {
            let key = normalize_key_event(WireKeyEvent {
                virtual_key,
                scan_code: 0,
                modifiers: 0,
                key_down: true,
                repeat: false,
            });
            assert_eq!(key.keycode, 0);
            assert_eq!(key.text, None);
        }
    }
}
