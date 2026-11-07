use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};

use crate::pal::{CY, LI};
use crate::{grad, hsh, live, pulse};

const NAMES: &[(&str, &str)] = &[
    ("cava", "cava"),
    ("lavat", "lavat"),
    ("pipes", "pipes.sh"),
    ("pipes.sh", "pipes.sh"),
    ("asciiquarium", "asciiquarium"),
    ("aquarium", "asciiquarium"),
    ("cbonsai", "cbonsai"),
    ("bonsai", "cbonsai"),
    ("cmatrix", "cmatrix"),
    ("matrix", "cmatrix"),
    ("unimatrix", "unimatrix"),
    ("neo-matrix", "neo-matrix"),
    ("neo", "neo-matrix"),
    ("tty-clock", "tty-clock"),
    ("clock", "tty-clock"),
    ("weathr", "weathr"),
    ("weather", "weathr"),
    ("rain", "terminal-rain"),
    ("terminal-rain", "terminal-rain"),
    ("globe", "globe"),
    ("stars", "stars"),
    ("starfield", "stars"),
];

pub const PICK: [&str; 13] = ["stars", "cava", "lavat", "pipes.sh", "asciiquarium", "cbonsai", "cmatrix", "unimatrix", "neo-matrix", "tty-clock", "weathr", "terminal-rain", "globe"];

pub fn avail() -> Vec<&'static str> {
    let mut v = vec!["off"];
    v.extend(PICK.iter().copied().filter(|k| exists(k)));
    v
}

pub const LIST: &str = "cava lavat pipes asciiquarium cbonsai cmatrix unimatrix neo-matrix tty-clock weathr rain globe stars";

pub fn canon(s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase();
    NAMES.iter().find(|(a, _)| *a == s).map(|(_, b)| *b)
}

pub fn exists(k: &str) -> bool {
    if k == "stars" {
        return true;
    }
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| std::path::Path::new(d).join(k).is_file())
}

fn argv(k: &'static str) -> Vec<&'static str> {
    match k {
        "cbonsai" => vec!["cbonsai", "-S"],
        "tty-clock" => vec!["tty-clock", "-c", "-s"],
        "weathr" => vec!["weathr", "--auto-location", "--hide-location", "--hide-hud", "--silent"],
        "globe" => vec!["globe", "-s", "-c", "1"],
        "neo-matrix" => vec!["neo-matrix", "-D", "--charset=ascii"],
        "cmatrix" => vec!["cmatrix", "-b"],
        k => vec![k],
    }
}

struct Pr {
    ch: Child,
    m: OwnedFd,
    ps: Arc<Mutex<vt100::Parser>>,
    sz: (u16, u16),
}

pub struct Bg {
    pub name: &'static str,
    pr: Option<Pr>,
}

fn ws(r: u16, c: u16) -> libc::winsize {
    libc::winsize { ws_row: r, ws_col: c, ws_xpixel: 0, ws_ypixel: 0 }
}

impl Bg {
    pub fn new(name: &'static str, rows: u16, cols: u16) -> io::Result<Bg> {
        if name == "stars" {
            return Ok(Bg { name, pr: None });
        }
        let (mut m, mut s) = (0, 0);
        let w = ws(rows, cols);
        if unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &w) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let m = unsafe { OwnedFd::from_raw_fd(m) };
        let s = unsafe { OwnedFd::from_raw_fd(s) };
        let av = argv(name);
        let mut c = Command::new(av[0]);
        c.args(&av[1..])
            .env("TERM", "xterm-256color")
            .env("COLUMNS", cols.to_string())
            .env("LINES", rows.to_string())
            .stdin(Stdio::from(s.try_clone()?))
            .stdout(Stdio::from(s.try_clone()?))
            .stderr(Stdio::from(s.try_clone()?));
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                libc::ioctl(0, libc::TIOCSCTTY, 0);
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let ch = c.spawn()?;
        drop(s);
        live::track_pid(ch.id() as i32);
        let ps = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        let p2 = ps.clone();
        let mut f = File::from(m.try_clone()?);
        std::thread::spawn(move || {
            let mut b = [0u8; 16384];
            while let Ok(n) = f.read(&mut b) {
                if n == 0 {
                    break;
                }
                if let Ok(mut p) = p2.lock() {
                    p.process(&b[..n]);
                }
            }
        });
        Ok(Bg { name, pr: Some(Pr { ch, m, ps, sz: (rows, cols) }) })
    }

    fn resize(&mut self, r: u16, c: u16) {
        let Some(p) = &mut self.pr else { return };
        if p.sz == (r, c) {
            return;
        }
        p.sz = (r, c);
        let w = ws(r, c);
        unsafe {
            libc::ioctl(p.m.as_raw_fd(), libc::TIOCSWINSZ, &w);
        }
        if let Ok(mut x) = p.ps.lock() {
            x.set_size(r, c);
        }
    }

    pub fn paint(&mut self, b: &mut Buffer, t: u64) {
        let a = b.area;
        self.resize(a.height, a.width);
        let occ: Vec<bool> = (0..a.height).flat_map(|y| (0..a.width).map(move |x| (x, y))).map(|(x, y)| b[(x, y)].symbol() != " " || b[(x, y)].bg != Color::Reset).collect();
        let at = |x: i32, y: u16| x >= 0 && (x as u16) < a.width && occ[y as usize * a.width as usize + x as usize];
        let free = |_: &Buffer, x: u16, y: u16| -> bool { !at(x as i32, y) && !at(x as i32 - 1, y) && !at(x as i32 + 1, y) };
        match &self.pr {
            None => {
                for y in 0..a.height {
                    for x in 0..a.width {
                        let sx = (x as u64 * 2 + t / 90) / 2;
                        let h = hsh(sx * 7919 + y as u64 * 104729, 17);
                        if h % 61 != 0 || !free(b, x, y) {
                            continue;
                        }
                        let tw = pulse(t + h % 200, 80.0 + (h % 60) as f32);
                        let ch = ["·", ".", "+", "*"][(h / 61 % 4) as usize];
                        let c = grad(&[Color::Rgb(0x2a, 0x22, 0x3c), if h % 3 == 0 { CY } else { LI }], 0.15 + tw * 0.35);
                        b[(x, y)].set_symbol(ch).set_style(Style::new().fg(c));
                    }
                }
            }
            Some(p) => {
                let Ok(ps) = p.ps.lock() else { return };
                let sc = ps.screen();
                for y in 0..a.height {
                    for x in 0..a.width {
                        let Some(cl) = sc.cell(y, x) else { continue };
                        let s = cl.contents();
                        let (s, col) = if s.trim().is_empty() {
                            match cl.bgcolor() {
                                vt100::Color::Default => continue,
                                c => {
                                    let (r, g, b) = vc(c);
                                    if r * 0.3 + g * 0.59 + b * 0.11 < 70.0 {
                                        continue;
                                    }
                                    ("█".to_string(), c)
                                }
                            }
                        } else {
                            (safe(&s, x as u64 * 31 + y as u64), cl.fgcolor())
                        };
                        if !free(b, x, y) {
                            continue;
                        }
                        b[(x, y)].set_symbol(&s).set_style(Style::new().fg(dim(vc(col), 0.12)));
                    }
                }
            }
        }
    }
}

impl Drop for Bg {
    fn drop(&mut self) {
        if let Some(p) = &mut self.pr {
            let id = p.ch.id() as i32;
            live::untrack_pid(id);
            unsafe {
                libc::kill(-id, libc::SIGTERM);
                libc::kill(id, libc::SIGTERM);
            }
            for _ in 0..10 {
                if matches!(p.ch.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            unsafe {
                libc::kill(-id, libc::SIGKILL);
            }
            let _ = p.ch.wait();
        }
    }
}

const P16: [(u8, u8, u8); 16] = [
    (0x22, 0x19, 0x2f),
    (0xf3, 0x8b, 0xa8),
    (0xa6, 0xe3, 0xa1),
    (0xf9, 0xe2, 0xaf),
    (0xb4, 0x97, 0xff),
    (0xd0, 0xa9, 0xff),
    (0x89, 0xdc, 0xeb),
    (0xd9, 0xd0, 0xee),
    (0x3a, 0x2b, 0x52),
    (0xff, 0x9f, 0xba),
    (0xb8, 0xf0, 0xb3),
    (0xff, 0xe7, 0xb8),
    (0xc5, 0xb3, 0xff),
    (0xe0, 0xbf, 0xff),
    (0xa7, 0xe7, 0xf2),
    (0xf7, 0xf2, 0xff),
];

fn safe(s: &str, k: u64) -> String {
    let c = s.chars().next().unwrap_or(' ');
    if c.is_ascii() || ('\u{2500}'..='\u{259f}').contains(&c) || ('\u{2800}'..='\u{28ff}').contains(&c) {
        return c.to_string();
    }
    let a = b"0123456789abcdefABCDEF$%#@&*+=";
    (a[(hsh(k, c as u64) % a.len() as u64) as usize] as char).to_string()
}

fn vc(c: vt100::Color) -> (f32, f32, f32) {
    let (r, g, b) = match c {
        vt100::Color::Default => (0xe9, 0xde, 0xfa),
        vt100::Color::Rgb(r, g, b) => (r, g, b),
        vt100::Color::Idx(i) if i < 16 => P16[i as usize],
        vt100::Color::Idx(i) if i < 232 => {
            let i = i - 16;
            let f = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (f(i / 36), f(i / 6 % 6), f(i % 6))
        }
        vt100::Color::Idx(i) => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    };
    (r as f32, g as f32, b as f32)
}

fn dim(c: (f32, f32, f32), k: f32) -> Color {
    let base = (26.0, 20.0, 38.0);
    let m = |x: f32, b: f32| (b + (x - b) * k) as u8;
    Color::Rgb(m(c.0, base.0), m(c.1, base.1), m(c.2, base.2))
}
