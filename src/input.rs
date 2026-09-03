use crate::capture::Geometry;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use rustvncserver::server::ServerEvent;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

unsafe extern "C" {
    fn vinny_set_clipboard(bytes: *const u8, length: usize);
}
use tokio::sync::mpsc;

pub async fn handle_events(
    mut events: mpsc::UnboundedReceiver<ServerEvent>,
    geometry: Arc<RwLock<Geometry>>,
) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(error) => {
            eprintln!("input unavailable: {error}");
            return;
        }
    };
    let mut buttons = HashMap::<usize, u8>::new();

    while let Some(event) = events.recv().await {
        match event {
            ServerEvent::PointerMove {
                client_id,
                x,
                y,
                button_mask,
            } => {
                let geometry = *geometry.read().expect("geometry lock");
                let absolute_x = geometry.origin_x
                    + (u32::from(x) * geometry.logical_width / u32::from(geometry.capture_width))
                        as i32;
                let absolute_y = geometry.origin_y
                    + (u32::from(y) * geometry.logical_height / u32::from(geometry.capture_height))
                        as i32;
                log_error(enigo.move_mouse(absolute_x, absolute_y, Coordinate::Abs));

                let previous = buttons.insert(client_id, button_mask).unwrap_or(0);
                update_button(&mut enigo, previous, button_mask, 0x01, Button::Left);
                update_button(&mut enigo, previous, button_mask, 0x02, Button::Middle);
                update_button(&mut enigo, previous, button_mask, 0x04, Button::Right);
                if button_mask & 0x08 != 0 && previous & 0x08 == 0 {
                    log_error(enigo.scroll(-1, Axis::Vertical));
                }
                if button_mask & 0x10 != 0 && previous & 0x10 == 0 {
                    log_error(enigo.scroll(1, Axis::Vertical));
                }
                if button_mask & 0x20 != 0 && previous & 0x20 == 0 {
                    log_error(enigo.scroll(-1, Axis::Horizontal));
                }
                if button_mask & 0x40 != 0 && previous & 0x40 == 0 {
                    log_error(enigo.scroll(1, Axis::Horizontal));
                }
            }
            ServerEvent::KeyPress { down, key, .. } => {
                if let Some(key) = keysym_to_key(key) {
                    log_error(enigo.key(key, direction(down)));
                }
            }
            ServerEvent::ExtendedKeyPress {
                down,
                keysym,
                keycode,
                ..
            } => {
                if let Some(keycode) = xt_to_macos_keycode(keycode) {
                    log_error(enigo.raw(keycode, direction(down)));
                } else if let Some(key) = keysym_to_key(keysym) {
                    log_error(enigo.key(key, direction(down)));
                }
            }
            ServerEvent::ClientDisconnected { client_id } => {
                if let Some(mask) = buttons.remove(&client_id) {
                    update_button(&mut enigo, mask, 0, 0x01, Button::Left);
                    update_button(&mut enigo, mask, 0, 0x02, Button::Middle);
                    update_button(&mut enigo, mask, 0, 0x04, Button::Right);
                }
            }
            ServerEvent::ClientConnected { client_id } => {
                eprintln!("VNC client {client_id} connected");
            }
            ServerEvent::CutText { text, .. } => unsafe {
                vinny_set_clipboard(text.as_ptr(), text.len());
            },
            ServerEvent::RfbMessageSent { .. } | ServerEvent::HandshakeComplete { .. } => {}
        }
    }
}

fn direction(down: bool) -> Direction {
    if down {
        Direction::Press
    } else {
        Direction::Release
    }
}

fn xt_to_macos_keycode(xt: u32) -> Option<u16> {
    Some(match xt {
        0x01 => 53, // Escape
        0x02 => 18,
        0x03 => 19,
        0x04 => 20,
        0x05 => 21,
        0x06 => 23,
        0x07 => 22,
        0x08 => 26,
        0x09 => 28,
        0x0a => 25,
        0x0b => 29,
        0x0c => 27,
        0x0d => 24,
        0x0e => 51,
        0x0f => 48,
        0x10 => 12,
        0x11 => 13,
        0x12 => 14,
        0x13 => 15,
        0x14 => 17,
        0x15 => 16,
        0x16 => 32,
        0x17 => 34,
        0x18 => 31,
        0x19 => 35,
        0x1a => 33,
        0x1b => 30,
        0x1c | 0x9c => 36,
        0x1d => 59,
        0x1e => 0,
        0x1f => 1,
        0x20 => 2,
        0x21 => 3,
        0x22 => 5,
        0x23 => 4,
        0x24 => 38,
        0x25 => 40,
        0x26 => 37,
        0x27 => 41,
        0x28 => 39,
        0x29 => 50,
        0x2a => 56,
        0x2b => 42,
        0x2c => 6,
        0x2d => 7,
        0x2e => 8,
        0x2f => 9,
        0x30 => 11,
        0x31 => 45,
        0x32 => 46,
        0x33 => 43,
        0x34 => 47,
        0x35 => 44,
        0x36 => 60,
        0x37 => 67,
        0x38 => 58,
        0x39 => 49,
        0x3a => 57,
        0x3b => 122,
        0x3c => 120,
        0x3d => 99,
        0x3e => 118,
        0x3f => 96,
        0x40 => 97,
        0x41 => 98,
        0x42 => 100,
        0x43 => 101,
        0x44 => 109,
        0x57 => 103,
        0x58 => 111,
        0x9d => 62,
        0xb8 => 61,
        0xc7 => 115,
        0xc8 => 126,
        0xc9 => 116,
        0xcb => 123,
        0xcd => 124,
        0xcf => 119,
        0xd0 => 125,
        0xd1 => 121,
        0xd2 => 114,
        0xd3 => 117,
        _ => return None,
    })
}

fn update_button(enigo: &mut Enigo, old: u8, new: u8, bit: u8, button: Button) {
    if old & bit == new & bit {
        return;
    }
    log_error(enigo.button(
        button,
        if new & bit != 0 {
            Direction::Press
        } else {
            Direction::Release
        },
    ));
}

fn log_error(result: enigo::InputResult<()>) {
    if let Err(error) = result {
        eprintln!("input error: {error}");
    }
}

fn keysym_to_key(keysym: u32) -> Option<Key> {
    let special = match keysym {
        0xff08 => Key::Backspace,
        0xff09 => Key::Tab,
        0xff0d => Key::Return,
        0xff1b => Key::Escape,
        0xff50 => Key::Home,
        0xff51 => Key::LeftArrow,
        0xff52 => Key::UpArrow,
        0xff53 => Key::RightArrow,
        0xff54 => Key::DownArrow,
        0xff55 => Key::PageUp,
        0xff56 => Key::PageDown,
        0xff57 => Key::End,
        0xffff => Key::Delete,
        0xffbe => Key::F1,
        0xffbf => Key::F2,
        0xffc0 => Key::F3,
        0xffc1 => Key::F4,
        0xffc2 => Key::F5,
        0xffc3 => Key::F6,
        0xffc4 => Key::F7,
        0xffc5 => Key::F8,
        0xffc6 => Key::F9,
        0xffc7 => Key::F10,
        0xffc8 => Key::F11,
        0xffc9 => Key::F12,
        0xffe1 | 0xffe2 => Key::Shift,
        0xffe3 | 0xffe4 => Key::Control,
        0xffe5 => Key::CapsLock,
        0xffe7 | 0xffe8 | 0xffeb | 0xffec => Key::Meta,
        0xffe9 | 0xffea => Key::Alt,
        _ => {
            let codepoint = if keysym & 0xff00_0000 == 0x0100_0000 {
                keysym & 0x00ff_ffff
            } else if keysym <= 0xff {
                keysym
            } else {
                return None;
            };
            return char::from_u32(codepoint).map(Key::Unicode);
        }
    };
    Some(special)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_vnc_keysyms() {
        assert_eq!(keysym_to_key(0xff0d), Some(Key::Return));
        assert_eq!(keysym_to_key(0x61), Some(Key::Unicode('a')));
        assert_eq!(keysym_to_key(0x0101_f642), Some(Key::Unicode('🙂')));
        assert_eq!(keysym_to_key(0x0200_0000), None);
        assert_eq!(xt_to_macos_keycode(0x1e), Some(0));
        assert_eq!(xt_to_macos_keycode(0xcd), Some(124));
        assert_eq!(xt_to_macos_keycode(0xffff), None);
    }
}
