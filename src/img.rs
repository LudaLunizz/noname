use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const MAX: usize = 3_750_000;

pub enum Clip {
    Img(Vec<u8>),
    Files(Vec<PathBuf>),
    Text(String),
    Empty,
}

pub fn mime(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub fn ext(m: &str) -> &'static str {
    match m {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}

pub fn b64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(T[(n >> 18) as usize & 63] as char);
        o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
}

fn unesc(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            if let Some(n) = it.next() {
                o.push(n);
            }
        } else {
            o.push(c);
        }
    }
    o
}

fn undec(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                o.push(v);
                i += 3;
                continue;
            }
        }
        o.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

fn split(s: &str) -> Vec<String> {
    let mut v = vec![];
    let mut cur = String::new();
    let mut q: Option<char> = None;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match (q, c) {
            (Some(x), c) if c == x => q = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => q = Some(c),
            (None, '\\') => {
                if let Some(n) = it.next() {
                    cur.push('\\');
                    cur.push(n);
                }
            }
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    v.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        v.push(cur);
    }
    v
}

fn norm(t: &str) -> PathBuf {
    let t = unesc(t);
    let t = match t.strip_prefix("file://") {
        Some(r) => undec(r.strip_prefix("localhost").unwrap_or(r)),
        None => t,
    };
    let t = match t.strip_prefix("~/") {
        Some(r) => format!("{}/{r}", crate::sys::home()),
        None => t,
    };
    PathBuf::from(t)
}

fn isimg(p: &Path) -> bool {
    let e = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(e.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") && p.is_file()
}

pub fn paths(s: &str) -> Vec<PathBuf> {
    let s = s.trim();
    if s.is_empty() || s.len() > 8192 {
        return vec![];
    }
    let whole = norm(s);
    if isimg(&whole) {
        return vec![whole];
    }
    let v: Vec<PathBuf> = split(s).iter().map(|t| norm(t)).collect();
    if !v.is_empty() && v.iter().all(|p| isimg(p)) { v } else { vec![] }
}

fn out(c: &mut Command) -> Option<Vec<u8>> {
    let o = c.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if o.status.success() && !o.stdout.is_empty() { Some(o.stdout) } else { None }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn pick(types: &str) -> Option<&'static str> {
    ["image/png", "image/jpeg", "image/webp", "image/gif"].into_iter().find(|t| types.lines().any(|l| l.trim() == *t))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn uris(b: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(b).lines().map(str::trim).filter(|l| l.starts_with("file://")).map(norm).filter(|p| isimg(p)).collect()
}

#[cfg(target_os = "macos")]
pub fn clip() -> Clip {
    let tmp = std::env::temp_dir().join(format!("noname-clip-{}.png", std::process::id()));
    let p = tmp.to_string_lossy().replace('"', "");
    let sc = format!(
        "try\nset d to (the clipboard as «class PNGf»)\non error\nreturn \"none\"\nend try\nset f to open for access (POSIX file \"{p}\") with write permission\nset eof f to 0\nwrite d to f\nclose access f\nreturn \"ok\""
    );
    if let Some(o) = out(Command::new("osascript").args(["-e", &sc])) {
        if String::from_utf8_lossy(&o).trim() == "ok" {
            if let Ok(b) = std::fs::read(&tmp) {
                let _ = std::fs::remove_file(&tmp);
                return Clip::Img(b);
            }
        }
    }
    match out(&mut Command::new("pbpaste")) {
        Some(b) => text(b),
        None => Clip::Empty,
    }
}

#[cfg(windows)]
pub fn clip() -> Clip {
    let tmp = std::env::temp_dir().join(format!("noname-clip-{}.png", std::process::id()));
    let p = tmp.to_string_lossy().replace('\'', "''");
    let sc = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $i=[System.Windows.Forms.Clipboard]::GetImage(); if($i){{$i.Save('{p}',[System.Drawing.Imaging.ImageFormat]::Png); 'ok'}} else {{ $f=[System.Windows.Forms.Clipboard]::GetFileDropList(); if($f.Count){{$f}} }}"
    );
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-STA", "-Command", &sc]);
    crate::sys::guard(&mut c);
    if let Some(o) = out(&mut c) {
        let s = String::from_utf8_lossy(&o).trim().to_string();
        if s == "ok" {
            if let Ok(b) = std::fs::read(&tmp) {
                let _ = std::fs::remove_file(&tmp);
                return Clip::Img(b);
            }
        }
        let v: Vec<PathBuf> = s.lines().map(|l| PathBuf::from(l.trim())).filter(|p| isimg(p)).collect();
        if !v.is_empty() {
            return Clip::Files(v);
        }
    }
    Clip::Empty
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn clip() -> Clip {
    let wl = std::env::var_os("WAYLAND_DISPLAY").is_some() && crate::sys::which("wl-paste").is_some() && !crate::sys::xclip_first();
    if wl {
        let ty = out(Command::new("wl-paste").arg("--list-types")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        if let Some(t) = pick(&ty) {
            if let Some(b) = out(Command::new("wl-paste").args(["--no-newline", "--type", t])) {
                return Clip::Img(b);
            }
        }
        if ty.lines().any(|l| l.trim() == "text/uri-list") {
            if let Some(b) = out(Command::new("wl-paste").args(["--no-newline", "--type", "text/uri-list"])) {
                let v = uris(&b);
                if !v.is_empty() {
                    return Clip::Files(v);
                }
            }
        }
        return match out(Command::new("wl-paste").arg("--no-newline")) {
            Some(b) => text(b),
            None => Clip::Empty,
        };
    }
    if crate::sys::which("xclip").is_some() {
        let ty = out(Command::new("xclip").args(["-selection", "clipboard", "-t", "TARGETS", "-o"])).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        if let Some(t) = pick(&ty) {
            if let Some(b) = out(Command::new("xclip").args(["-selection", "clipboard", "-t", t, "-o"])) {
                return Clip::Img(b);
            }
        }
        if ty.lines().any(|l| l.trim() == "text/uri-list") {
            if let Some(b) = out(Command::new("xclip").args(["-selection", "clipboard", "-t", "text/uri-list", "-o"])) {
                let v = uris(&b);
                if !v.is_empty() {
                    return Clip::Files(v);
                }
            }
        }
        return match out(Command::new("xclip").args(["-selection", "clipboard", "-o"])) {
            Some(b) => text(b),
            None => Clip::Empty,
        };
    }
    Clip::Empty
}

#[allow(dead_code)]
fn text(b: Vec<u8>) -> Clip {
    let s = String::from_utf8_lossy(&b).into_owned();
    if s.is_empty() { Clip::Empty } else { Clip::Text(s) }
}

pub fn shrink(b: Vec<u8>) -> Result<(Vec<u8>, &'static str), usize> {
    let Some(m) = mime(&b) else { return Err(0) };
    if b.len() <= MAX {
        return Ok((b, m));
    }
    let n = b.len();
    if crate::sys::which("ffmpeg").is_none() {
        return Err(n);
    }
    let d = std::env::temp_dir();
    let id = std::process::id();
    let src = d.join(format!("noname-big-{id}.{}", ext(m)));
    if std::fs::write(&src, &b).is_err() {
        return Err(n);
    }
    let mut best = Err(n);
    for (px, q) in [(2400, "3"), (1800, "5"), (1200, "8")] {
        let dst = d.join(format!("noname-small-{id}.jpg"));
        let vf = format!("scale='min({px},iw)':-2");
        let ok = Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&src)
            .args(["-vf", &vf, "-frames:v", "1", "-q:v", q])
            .arg(&dst)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        let r = if ok { std::fs::read(&dst).ok() } else { None };
        let _ = std::fs::remove_file(&dst);
        if let Some(r) = r {
            if r.len() <= MAX {
                best = Ok((r, "image/jpeg"));
                break;
            }
        }
    }
    let _ = std::fs::remove_file(&src);
    best
}
