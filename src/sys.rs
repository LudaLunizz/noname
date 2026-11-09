use std::path::PathBuf;
use std::process::{Child, Command};

pub fn home() -> String {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default()
}

pub fn cfgdir() -> String {
    #[cfg(windows)]
    if let Ok(a) = std::env::var("APPDATA") {
        return format!("{a}\\noname");
    }
    let b = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home()));
    format!("{b}/noname")
}

pub fn cfgfile(n: &str) -> String {
    let d = cfgdir();
    if cfg!(windows) { format!("{d}\\{n}") } else { format!("{d}/{n}") }
}

pub fn bin(b: &str) -> std::ffi::OsString {
    which(b).map(|p| p.into_os_string()).unwrap_or_else(|| b.into())
}

#[cfg(unix)]
pub fn which(b: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let h = home();
    let mut ds: Vec<String> = std::env::var("PATH").unwrap_or_default().split(':').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
    for d in [format!("{h}/.cargo/bin"), format!("{h}/.local/bin"), format!("{h}/bin"), "/usr/local/bin".into(), "/usr/games".into(), "/usr/local/games".into(), "/opt/homebrew/bin".into(), "/home/linuxbrew/.linuxbrew/bin".into(), "/snap/bin".into()] {
        if !ds.contains(&d) {
            ds.push(d);
        }
    }
    ds.into_iter().map(|d| std::path::Path::new(&d).join(b)).find(|p| p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

#[cfg(windows)]
pub fn which(b: &str) -> Option<PathBuf> {
    let h = home();
    let mut ds: Vec<String> = std::env::var("PATH").unwrap_or_default().split(';').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
    let mut extra = vec![format!("{h}\\.local\\bin"), format!("{h}\\.cargo\\bin")];
    if let Ok(a) = std::env::var("APPDATA") {
        extra.push(format!("{a}\\npm"));
    }
    for d in extra {
        if !ds.contains(&d) {
            ds.push(d);
        }
    }
    let exts: Vec<String> = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()).split(';').filter(|x| !x.is_empty()).map(|x| x.to_lowercase()).collect();
    let has_ext = std::path::Path::new(b).extension().is_some();
    for d in ds {
        let base = std::path::Path::new(&d).join(b);
        if has_ext && base.is_file() {
            return Some(base);
        }
        for e in &exts {
            let p = std::path::Path::new(&d).join(format!("{b}{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(unix)]
pub fn guard(c: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    unsafe {
        c.pre_exec(|| {
            #[cfg(target_os = "linux")]
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    c.process_group(0)
}

#[cfg(windows)]
pub fn guard(c: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000 | 0x0000_0200)
}

#[cfg(unix)]
pub fn adopt(_: &Child) {}

#[cfg(windows)]
pub fn adopt(c: &Child) {
    use std::os::windows::io::AsRawHandle;
    let j = job();
    if j != 0 {
        unsafe {
            win::AssignProcessToJobObject(j as _, c.as_raw_handle() as _);
        }
    }
}

#[cfg(unix)]
pub fn term(id: i32) {
    unsafe {
        libc::kill(-id, libc::SIGTERM);
        libc::kill(id, libc::SIGTERM);
    }
}

#[cfg(unix)]
pub fn kill(id: i32) {
    unsafe {
        libc::kill(-id, libc::SIGKILL);
        libc::kill(id, libc::SIGKILL);
    }
}

#[cfg(windows)]
pub fn term(id: i32) {
    kill(id)
}

#[cfg(windows)]
pub fn kill(id: i32) {
    use std::os::windows::process::CommandExt;
    let _ = Command::new("taskkill")
        .args(["/PID", &id.to_string(), "/T", "/F"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .status();
}

pub fn exit_now() -> ! {
    #[cfg(unix)]
    unsafe {
        libc::_exit(0)
    }
    #[cfg(windows)]
    std::process::exit(0)
}

#[cfg(unix)]
pub fn on_close(f: fn()) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    if let Ok(mut sg) = signal_hook::iterator::Signals::new([SIGHUP, SIGTERM, SIGINT]) {
        std::thread::spawn(move || {
            if sg.forever().next().is_some() {
                f();
                exit_now();
            }
        });
    }
}

#[cfg(windows)]
pub fn on_close(f: fn()) {
    let _ = job();
    let _ = HANDLER.set(f);
    unsafe extern "system" fn h(_: u32) -> i32 {
        if let Some(f) = HANDLER.get() {
            f();
        }
        exit_now()
    }
    unsafe {
        win::SetConsoleCtrlHandler(Some(h), 1);
    }
}

#[cfg(windows)]
static HANDLER: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();

#[cfg(windows)]
fn job() -> isize {
    static J: std::sync::OnceLock<isize> = std::sync::OnceLock::new();
    *J.get_or_init(|| unsafe {
        let j = win::CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if j.is_null() {
            return 0;
        }
        let mut info: win::JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = win::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        win::SetInformationJobObject(j, win::JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of_val(&info) as u32);
        j as isize
    })
}

#[cfg(windows)]
mod win {
    pub use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
    pub use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
}

pub fn open(path: &str, line: &str) {
    let mut c = if let Ok(t) = std::env::var("NONAME_OPEN") {
        let t = t.replace("{line}", if line.is_empty() { "1" } else { line });
        let t = if t.contains("{}") { t.replace("{}", path) } else { format!("{t} {path}") };
        let mut v = t.split_whitespace().map(str::to_string);
        let mut c = Command::new(v.next().unwrap_or_default());
        c.args(v);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", "start", "", path]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let _ = c.spawn();
}

pub fn month() -> i32 {
    #[cfg(unix)]
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm.tm_mon
    }
    #[cfg(windows)]
    unsafe {
        let mut st: windows_sys::Win32::Foundation::SYSTEMTIME = std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st);
        st.wMonth as i32 - 1
    }
}

/// Puts text on the system clipboard. Returns false when no clipboard tool was found,
/// so the caller can say that only the terminal's own clipboard (OSC 52) was tried.
pub fn setclip(t: &str) -> bool {
    let c: Option<(&str, &[&str])> = if cfg!(target_os = "macos") {
        Some(("pbcopy", &[]))
    } else if cfg!(windows) {
        None
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() && which("wl-copy").is_some() {
        Some(("wl-copy", &[]))
    } else if which("xclip").is_some() {
        Some(("xclip", &["-selection", "clipboard"]))
    } else {
        None
    };
    let Some((p, a)) = c else { return cfg!(windows) };
    let Ok(mut ch) = Command::new(p).args(a).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { return false };
    if let Some(mut i) = ch.stdin.take() {
        use std::io::Write;
        let _ = i.write_all(t.as_bytes());
    }
    std::thread::spawn(move || ch.wait());
    true
}

