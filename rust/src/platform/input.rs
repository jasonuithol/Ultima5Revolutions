//! Keyboard: a BIOS-style key buffer (AH = scan code, AL = ASCII; AL = 0 for extended keys)
//! fed from minifb events.
use minifb::{Key, Window};
use std::collections::VecDeque;

pub struct KeyQueue {
    pub queue: VecDeque<u16>,
    prev: Vec<Key>,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub caps: bool,
}

fn scan_code(k: Key) -> u8 {
    use Key::*;
    match k {
        Escape => 0x01, Key1 => 0x02, Key2 => 0x03, Key3 => 0x04, Key4 => 0x05, Key5 => 0x06, Key6 => 0x07,
        Key7 => 0x08, Key8 => 0x09, Key9 => 0x0a, Key0 => 0x0b, Minus => 0x0c, Equal => 0x0d, Backspace => 0x0e,
        Tab => 0x0f, Q => 0x10, W => 0x11, E => 0x12, R => 0x13, T => 0x14, Y => 0x15, U => 0x16, I => 0x17,
        O => 0x18, P => 0x19, LeftBracket => 0x1a, RightBracket => 0x1b, Enter => 0x1c, A => 0x1e, S => 0x1f,
        D => 0x20, F => 0x21, G => 0x22, H => 0x23, J => 0x24, K => 0x25, L => 0x26, Semicolon => 0x27,
        Apostrophe => 0x28, Backquote => 0x29, Backslash => 0x2b, Z => 0x2c, X => 0x2d, C => 0x2e, V => 0x2f,
        B => 0x30, N => 0x31, M => 0x32, Comma => 0x33, Period => 0x34, Slash => 0x35, NumPadAsterisk => 0x37,
        Space => 0x39, F1 => 0x3b, F2 => 0x3c, F3 => 0x3d, F4 => 0x3e, F5 => 0x3f, F6 => 0x40, F7 => 0x41,
        F8 => 0x42, F9 => 0x43, F10 => 0x44, Home => 0x47, Up => 0x48, PageUp => 0x49, NumPadMinus => 0x4a,
        Left => 0x4b, Right => 0x4d, NumPadPlus => 0x4e, End => 0x4f, Down => 0x50, PageDown => 0x51,
        Insert => 0x52, Delete => 0x53, NumPad7 => 0x47, NumPad8 => 0x48, NumPad9 => 0x49, NumPad4 => 0x4b,
        NumPad5 => 0x4c, NumPad6 => 0x4d, NumPad1 => 0x4f, NumPad2 => 0x50, NumPad3 => 0x51, NumPad0 => 0x52,
        NumPadDot => 0x53, NumPadSlash => 0x35, NumPadEnter => 0x1c, _ => 0,
    }
}

fn ascii_of(k: Key, shift: bool, ctrl: bool, caps: bool) -> u8 {
    use Key::*;
    let letter = |c: u8| -> u8 {
        if ctrl { return c - b'a' + 1; }
        if shift ^ caps { c - 32 } else { c }
    };
    let pair = |n: u8, s: u8| if shift { s } else { n };
    match k {
        A => letter(b'a'), B => letter(b'b'), C => letter(b'c'), D => letter(b'd'), E => letter(b'e'), F => letter(b'f'),
        G => letter(b'g'), H => letter(b'h'), I => letter(b'i'), J => letter(b'j'), K => letter(b'k'), L => letter(b'l'),
        M => letter(b'm'), N => letter(b'n'), O => letter(b'o'), P => letter(b'p'), Q => letter(b'q'), R => letter(b'r'),
        S => letter(b's'), T => letter(b't'), U => letter(b'u'), V => letter(b'v'), W => letter(b'w'), X => letter(b'x'),
        Y => letter(b'y'), Z => letter(b'z'),
        Key1 => pair(b'1', b'!'), Key2 => pair(b'2', b'@'), Key3 => pair(b'3', b'#'), Key4 => pair(b'4', b'$'),
        Key5 => pair(b'5', b'%'), Key6 => pair(b'6', b'^'), Key7 => pair(b'7', b'&'), Key8 => pair(b'8', b'*'),
        Key9 => pair(b'9', b'('), Key0 => pair(b'0', b')'), Minus => pair(b'-', b'_'), Equal => pair(b'=', b'+'),
        LeftBracket => pair(b'[', b'{'), RightBracket => pair(b']', b'}'), Semicolon => pair(b';', b':'),
        Apostrophe => pair(b'\'', b'"'), Backquote => pair(b'`', b'~'), Backslash => pair(b'\\', b'|'),
        Comma => pair(b',', b'<'), Period => pair(b'.', b'>'), Slash => pair(b'/', b'?'),
        Space => b' ', Enter | NumPadEnter => 13, Escape => 27, Backspace => 8, Tab => 9,
        NumPadAsterisk => b'*', NumPadMinus => b'-', NumPadPlus => b'+', NumPadSlash => b'/',
        // keypad digits report their ASCII digit (NumLock on) so the game sees '1'..'9'
        NumPad0 => b'0', NumPad1 => b'1', NumPad2 => b'2', NumPad3 => b'3', NumPad4 => b'4', NumPad5 => b'5',
        NumPad6 => b'6', NumPad7 => b'7', NumPad8 => b'8', NumPad9 => b'9', NumPadDot => b'.',
        _ => 0,
    }
}

impl KeyQueue {
    pub fn new() -> KeyQueue { KeyQueue { queue: VecDeque::new(), prev: Vec::new(), ctrl: false, shift: false, alt: false, caps: false } }
    /// Poll the window and translate newly pressed keys into BIOS key codes.
    pub fn poll(&mut self, win: &Window) {
        let keys = win.get_keys();
        self.ctrl = keys.contains(&Key::LeftCtrl) || keys.contains(&Key::RightCtrl);
        self.shift = keys.contains(&Key::LeftShift) || keys.contains(&Key::RightShift);
        self.alt = keys.contains(&Key::LeftAlt) || keys.contains(&Key::RightAlt);
        let pressed = win.get_keys_pressed(minifb::KeyRepeat::Yes);
        for k in pressed {
            if k == Key::CapsLock { self.caps = !self.caps; continue; }
            let sc = scan_code(k);
            if sc == 0 { continue; }
            let is_keypad = matches!(k, Key::NumPad0 | Key::NumPad1 | Key::NumPad2 | Key::NumPad3 | Key::NumPad4 | Key::NumPad5 | Key::NumPad6 | Key::NumPad7 | Key::NumPad8 | Key::NumPad9 | Key::NumPadDot);
            let asc = if is_keypad { ascii_of(k, false, false, false) } else { ascii_of(k, self.shift, self.ctrl, self.caps) };
            self.queue.push_back(((sc as u16) << 8) | asc as u16);
        }
        self.prev = keys;
    }
    /// INT 16h AH=1: key available?
    pub fn available(&self) -> Option<u16> { self.queue.front().copied() }
    /// INT 16h AH=0: take next key.
    pub fn take(&mut self) -> Option<u16> { self.queue.pop_front() }
    pub fn flush(&mut self) { self.queue.clear(); }
    /// INT 16h AH=2 shift flags (bit 0 right shift, 1 left shift, 2 ctrl, 3 alt, 6 caps lock).
    pub fn shift_flags(&self) -> u8 {
        (self.shift as u8) * 3 | (self.ctrl as u8) << 2 | (self.alt as u8) << 3 | (self.caps as u8) << 6
    }
}

/// Parse a key name for scripted input: single characters, "ENTER", "ESC", "SPACE", "UP"/"DOWN"/"LEFT"/"RIGHT", "F1".."F10", "BS".
pub fn parse_key_name(name: &str) -> Option<u16> {
    let up = name.to_ascii_uppercase();
    let special = match up.as_str() {
        "ENTER" => Some(0x1c0d), "ESC" => Some(0x011b), "SPACE" => Some(0x3920), "BS" => Some(0x0e08), "TAB" => Some(0x0f09),
        "UP" => Some(0x4800), "DOWN" => Some(0x5000), "LEFT" => Some(0x4b00), "RIGHT" => Some(0x4d00),
        "HOME" => Some(0x4700), "END" => Some(0x4f00), "PGUP" => Some(0x4900), "PGDN" => Some(0x5100),
        _ => None,
    };
    if special.is_some() { return special; }
    if let Some(n) = up.strip_prefix('F') { if let Ok(i) = n.parse::<u8>() { if (1..=10).contains(&i) { return Some(((0x3a + i as u16) << 8)); } } }
    if name.len() == 1 {
        let c = name.as_bytes()[0];
        let k = match c.to_ascii_lowercase() { b'a'..=b'z' => Key::A as u8 + (c.to_ascii_lowercase() - b'a'), _ => 0 };
        let _ = k;
        return Some(c as u16); // scan code 0 is fine for ASCII keys
    }
    None
}
