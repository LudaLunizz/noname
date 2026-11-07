use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use crate::sys;

pub const REV: &str = env!("NONAME_REV");
pub const DIRTY: bool = !env!("NONAME_DIRTY").is_empty();
pub const DATE: &str = env!("NONAME_DATE");
const URL: &str = env!("CARGO_PKG_REPOSITORY");
const EVERY: u64 = 6 * 3600;

pub enum Up {
    Current,
    Ready(String),
    Skip(String),
    Fail(String),
}

pub fn short() -> String {
    let r: String = REV.chars().take(7).collect();
    let r = if r.is_empty() { "dev".to_string() } else { r };
    if DIRTY { format!("{r}+") } else { r }
}

pub fn enabled() -> bool {
    std::env::var("NONAME_NO_UPDATE").is_err() && std::fs::read_to_string(sys::cfgfile("autoupdate")).map(|s| s.trim() != "off").unwrap_or(true)
}

pub fn set_enabled(on: bool) {
    let f = sys::cfgfile("autoupdate");
    if let Some(d) = Path::new(&f).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&f, if on { "on" } else { "off" });
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn due() -> bool {
    let last = std::fs::read_to_string(sys::cfgfile("update-check")).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
    now().saturating_sub(last) >= EVERY
}

fn cache() -> PathBuf {
    #[cfg(windows)]
    if let Ok(a) = std::env::var("LOCALAPPDATA") {
        return Path::new(&a).join("noname").join("src");
    }
    let b = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{}/.cache", sys::home()));
    Path::new(&b).join("noname").join("src")
}

fn quiet(c: &mut Command) -> &mut Command {
    sys::guard(c).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).env("GIT_TERMINAL_PROMPT", "0")
}

fn ok(c: &mut Command) -> bool {
    match quiet(c).spawn() {
        Ok(mut ch) => {
            crate::live::track_child(&ch);
            let r = ch.wait().is_ok_and(|s| s.success());
            crate::live::untrack_pid(ch.id() as i32);
            r
        }
        Err(_) => false,
    }
}

fn out(c: &mut Command) -> String {
    sys::guard(c).stdin(Stdio::null()).stderr(Stdio::null()).env("GIT_TERMINAL_PROMPT", "0").output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

pub fn cleanup() {
    #[cfg(windows)]
    if let Ok(e) = std::env::current_exe() {
        let _ = std::fs::remove_file(e.with_extension("old.exe"));
    }
}

pub fn check(force: bool) -> Receiver<Up> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(force));
    });
    rx
}

fn run(force: bool) -> Up {
    if DIRTY && !force {
        return Up::Skip("local build with uncommitted changes".into());
    }
    let (Some(git), Some(cargo)) = (sys::which("git"), sys::which("cargo")) else {
        return Up::Skip("needs git and cargo".into());
    };
    let c = cache();
    let f = sys::cfgfile("update-check");
    if let Some(d) = Path::new(&f).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&f, now().to_string());
    if c.join(".git").is_dir() {
        if !ok(Command::new(&git).arg("-C").arg(&c).args(["fetch", "--quiet", "origin"])) {
            return Up::Fail("could not reach github".into());
        }
    } else {
        let _ = std::fs::remove_dir_all(&c);
        if let Some(p) = c.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if !ok(Command::new(&git).args(["clone", "--quiet", URL]).arg(&c)) {
            return Up::Fail("could not reach github".into());
        }
    }
    let remote = out(Command::new(&git).arg("-C").arg(&c).args(["rev-parse", "origin/HEAD"]));
    if remote.is_empty() {
        return Up::Fail("no remote head".into());
    }
    if remote == REV {
        return Up::Current;
    }
    if !REV.is_empty() && !ok(Command::new(&git).arg("-C").arg(&c).args(["merge-base", "--is-ancestor", REV, &remote])) {
        return Up::Skip("this build is newer than github".into());
    }
    if !ok(Command::new(&git).arg("-C").arg(&c).args(["checkout", "--quiet", "--force", &remote])) {
        return Up::Fail("checkout failed".into());
    }
    if !ok(Command::new(&cargo).args(["build", "--release", "--locked", "--quiet"]).current_dir(&c)) {
        return Up::Fail("build failed".into());
    }
    let nb = c.join("target").join("release").join(if cfg!(windows) { "noname.exe" } else { "noname" });
    let Ok(me) = std::env::current_exe() else { return Up::Fail("unknown install path".into()) };
    if let Err(e) = swap(&nb, &me) {
        return Up::Fail(format!("install failed: {e}"));
    }
    Up::Ready(remote.chars().take(7).collect())
}

fn swap(nb: &Path, me: &Path) -> std::io::Result<()> {
    let tmp = me.with_extension("new");
    std::fs::copy(nb, &tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(windows)]
    {
        let old = me.with_extension("old.exe");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(me, &old)?;
    }
    std::fs::rename(&tmp, me)
}
