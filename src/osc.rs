//! Terminal background detection for `theme = "auto"`.
//!
//! Asks the terminal for its background color with OSC 11 and falls back to
//! `COLORFGBG`. Terminals that do not answer simply yield `None`.

use std::time::Duration;

/// `Some(true)` for a dark background, `Some(false)` for a light one.
pub fn terminal_is_dark() -> Option<bool> {
    query_background(Duration::from_millis(250))
        .and_then(|reply| parse_osc11(&reply))
        .map(|(r, g, b)| luminance(r, g, b) < 0.5)
        .or_else(|| colorfgbg(std::env::var("COLORFGBG").ok().as_deref()))
}

/// Relative luminance of 8-bit sRGB components (Rec. 709 weights).
pub fn luminance(r: u8, g: u8, b: u8) -> f32 {
    (0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)) / 255.0
}

/// Parses `ESC ] 11 ; rgb:RRRR/GGGG/BBBB (BEL | ESC \)`. Components may have
/// one to four hex digits.
pub fn parse_osc11(reply: &str) -> Option<(u8, u8, u8)> {
    let start = reply.find("rgb:")? + 4;
    let body: String = reply[start..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == '/')
        .collect();

    let mut parts = body.split('/');
    let mut component = || -> Option<u8> {
        let part = parts.next().filter(|p| (1..=4).contains(&p.len()))?;
        let value = u32::from_str_radix(part, 16).ok()?;
        let max = 16u32.pow(part.len() as u32) - 1;
        Some(((value * 255 + max / 2) / max) as u8)
    };

    let (r, g, b) = (component()?, component()?, component()?);
    parts.next().is_none().then_some((r, g, b))
}

/// `COLORFGBG="fg;bg"`: backgrounds 0-6 and 8 are dark, 7 and 9-15 light.
pub fn colorfgbg(value: Option<&str>) -> Option<bool> {
    let background: u8 = value?.rsplit(';').next()?.trim().parse().ok()?;
    match background {
        0..=6 | 8 => Some(true),
        7 | 9..=15 => Some(false),
        _ => None,
    }
}

#[cfg(unix)]
fn query_background(timeout: Duration) -> Option<String> {
    use std::fs::OpenOptions;
    use std::io::{IsTerminal, Read, Write};
    use std::os::fd::AsRawFd;

    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return None;
    }

    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;

    // Raw mode through termios directly: crossterm keeps global state about
    // the "original" mode and the interface enables it again right after.
    // SAFETY: `termios` is plain data and `fd` is the open /dev/tty descriptor.
    let saved = unsafe {
        let mut current: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(tty.as_raw_fd(), &mut current) != 0 {
            return None;
        }
        let saved = current;
        libc::cfmakeraw(&mut current);
        // Reads time out instead of blocking: poll(2) is unreliable on macOS
        // terminal devices, so the read timeout lives in the line discipline.
        current.c_cc[libc::VMIN] = 0;
        current.c_cc[libc::VTIME] = 1;
        if libc::tcsetattr(tty.as_raw_fd(), libc::TCSANOW, &current) != 0 {
            return None;
        }
        saved
    };

    let result = (|| {
        tty.write_all(b"\x1b]11;?\x07").ok()?;
        tty.flush().ok()?;

        let mut reply = Vec::new();
        let mut buffer = [0u8; 64];
        // Each read waits at most VTIME (100 ms); the deadline bounds the total.
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            let read = tty.read(&mut buffer).ok()?;
            if read == 0 {
                continue;
            }
            reply.extend_from_slice(&buffer[..read]);
            if reply.contains(&0x07) || reply.windows(2).any(|pair| pair == b"\x1b\\") {
                break;
            }
        }
        String::from_utf8(reply).ok()
    })();

    // Whatever is still queued (a late or partial reply) must not reach the
    // interface as key presses.
    // SAFETY: the descriptor belongs to `tty`, which is still open here.
    unsafe {
        libc::tcflush(tty.as_raw_fd(), libc::TCIFLUSH);
    }
    // SAFETY: restores the attributes read above on the same descriptor.
    unsafe {
        libc::tcsetattr(tty.as_raw_fd(), libc::TCSANOW, &saved);
    }
    result.filter(|reply| reply.contains("rgb:"))
}

#[cfg(not(unix))]
fn query_background(_timeout: Duration) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_four_digit_replies() {
        assert_eq!(
            parse_osc11("\x1b]11;rgb:1f1f/1f1f/2828\x07"),
            Some((0x1f, 0x1f, 0x28))
        );
        assert_eq!(
            parse_osc11("\x1b]11;rgb:ffff/ffff/ffff\x1b\\"),
            Some((255, 255, 255))
        );
    }

    #[test]
    fn parses_short_components() {
        assert_eq!(parse_osc11("rgb:f/0/8"), Some((255, 0, 136)));
        assert_eq!(parse_osc11("rgb:ff/00/80"), Some((255, 0, 128)));
    }

    #[test]
    fn rejects_malformed_replies() {
        assert_eq!(parse_osc11(""), None);
        assert_eq!(parse_osc11("rgb:12345/00/00"), None);
        assert_eq!(parse_osc11("rgb:ff/ff"), None);
        assert_eq!(parse_osc11("rgb:ff/ff/ff/ff"), None);
        assert_eq!(parse_osc11("rgb:zz/00/00"), None);
    }

    #[test]
    fn luminance_splits_dark_from_light() {
        assert!(luminance(0x1f, 0x1f, 0x28) < 0.5);
        assert!(luminance(0xf2, 0xec, 0xbc) > 0.5);
    }

    #[test]
    fn colorfgbg_is_interpreted() {
        assert_eq!(colorfgbg(Some("15;0")), Some(true));
        assert_eq!(colorfgbg(Some("0;15")), Some(false));
        assert_eq!(colorfgbg(Some("0;default;7")), Some(false));
        assert_eq!(colorfgbg(Some("garbage")), None);
        assert_eq!(colorfgbg(None), None);
    }
}
