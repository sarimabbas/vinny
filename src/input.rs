use crate::capture::Geometry;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use rustvncserver::server::ServerEvent;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub async fn handle_events(mut events: mpsc::UnboundedReceiver<ServerEvent>, geometry: Geometry) {
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
                    log_error(enigo.key(
                        key,
                        if down {
                            Direction::Press
                        } else {
                            Direction::Release
                        },
                    ));
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
            ServerEvent::CutText { .. }
            | ServerEvent::RfbMessageSent { .. }
            | ServerEvent::HandshakeComplete { .. } => {}
        }
    }
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
    }
}
