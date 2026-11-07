pub mod bg;
pub mod sys;
mod codex;
mod live;

use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers as Km, KeyboardEnhancementFlags, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use ratatui::crossterm::event::{KeyCode as K, KeyEvent as Ke, KeyModifiers as M};

#[allow(non_snake_case)]
pub mod pal {
    use ratatui::style::Color;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub struct Theme {
        pub name: &'static str,
        pub desc: &'static str,
        c: [u32; 12],
    }

    pub const THEMES: [Theme; 4] = [
        Theme { name: "default", desc: "white, gray, light purple", c: [0xe4e4e8, 0xffffff, 0xb4b4bc, 0x85858f, 0xc3b0ff, 0xd8caff, 0xcdc4e8, 0xa9d6e5, 0xa6e3a1, 0xf38ba8, 0xe6b3ff, 0x161618] },
        Theme { name: "dracula", desc: "purple, pink, neon", c: [0xf8f8f2, 0xffffff, 0xc3c6dd, 0x7b88b8, 0xbd93f9, 0xff92df, 0xcaa9fa, 0x8be9fd, 0x50fa7b, 0xff5555, 0xff79c6, 0x282a36] },
        Theme { name: "tokyo-night", desc: "night blue, city lights", c: [0xc0caf5, 0xe0e6ff, 0xa9b1d6, 0x737aa2, 0x7aa2f7, 0xbb9af7, 0xb4c2f0, 0x7dcfff, 0x9ece6a, 0xf7768e, 0xff9e64, 0x1a1b26] },
        Theme { name: "mocha", desc: "soft pastels on dark roast", c: [0xcdd6f4, 0xeef1fc, 0xbac2de, 0x9399b2, 0xcba6f7, 0xb4befe, 0xf5c2e7, 0x89dceb, 0xa6e3a1, 0xf38ba8, 0xf5c2e7, 0x1e1e2e] },
    ];

    static CUR: AtomicUsize = AtomicUsize::new(0);

    pub fn cur() -> usize {
        CUR.load(Ordering::Relaxed)
    }
    pub fn set(i: usize) {
        CUR.store(i.min(THEMES.len() - 1), Ordering::Relaxed);
    }
    pub fn find(n: &str) -> Option<usize> {
        let k = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        let n = k(n);
        THEMES.iter().position(|t| k(t.name) == n || (n == "catppuccin" && t.name == "mocha") || (n == "tokyo" && t.name == "tokyo-night"))
    }
    pub fn file() -> String {
        crate::sys::cfgfile("theme")
    }
    pub fn load() {
        if let Some(i) = std::env::var("NONAME_THEME").ok().or_else(|| std::fs::read_to_string(file()).ok()).and_then(|x| find(&x)) {
            set(i);
        }
    }
    pub fn save() {
        let f = file();
        if let Some(d) = std::path::Path::new(&f).parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&f, THEMES[cur()].name);
    }
    fn c(i: usize) -> Color {
        let v = THEMES[cur()].c[i];
        Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub fn FG() -> Color { c(0) }
    pub fn WH() -> Color { c(1) }
    pub fn MU() -> Color { c(2) }
    pub fn FA() -> Color { c(3) }
    pub fn LI() -> Color { c(4) }
    pub fn L2() -> Color { c(5) }
    pub fn PE() -> Color { c(6) }
    pub fn CY() -> Color { c(7) }
    pub fn GR() -> Color { c(8) }
    pub fn RD() -> Color { c(9) }
    pub fn PK() -> Color { c(10) }
    pub fn BASE() -> Color { c(11) }
}


pub const MODELS: [&str; 4] = ["opus 5.5", "sonnet 5.5", "haiku 5.5", "fable 5.1"];
pub const EFF: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
pub const MODES: [&str; 4] = ["default", "plan", "auto", "bypass"];
pub const CMDS: [(&str, &str); 9] = [
    ("btw", "side question, main chat keeps going"),
    ("clear", "new session, empty context"),
    ("compact", "summarize to free context"),
    ("model", "switch model"),
    ("effort", "switch effort level"),
    ("context", "show context usage"),
    ("resume", "pick an older session"),
    ("usage", "plan limits and stats"),
    ("help", "keys and commands"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tk {
    Read,
    Edit,
    Bash,
    Grep,
    Web,
    Task,
    Oth,
}
impl Tk {
    pub fn nm(&self) -> &'static str {
        match self {
            Tk::Read => "read",
            Tk::Edit => "edit",
            Tk::Bash => "bash",
            Tk::Grep => "grep",
            Tk::Web => "web",
            Tk::Task => "task",
            Tk::Oth => "tool",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum St {
    Run,
    Ok,
    Err,
}

#[derive(Clone)]
pub enum Ln {
    P(String),
    A(usize, String),
    D(usize, String),
    C(usize, String),
}

#[derive(Clone)]
pub struct Tool {
    pub k: Tk,
    pub nm: String,
    pub a: String,
    pub sum: String,
    pub body: Vec<Ln>,
    pub st: St,
    pub t0: u64,
    pub t1: u64,
}

#[derive(Clone)]
pub struct Ask {
    pub cmd: String,
    pub ans: Option<char>,
}

#[derive(Clone)]
pub enum B {
    U(String, String),
    T(String),
    Th(String),
    Tl(Tool),
    Ask(Ask),
    Sys(String),
    Rc(String, u64),
}

enum Sp {
    Th(String),
    Tx(String),
    Tl(Tool, u32),
    Fin,
    Ask(String),
    Todo(Vec<(u8, String)>),
    Rc(String),
}

pub struct Btw {
    pub q: String,
    pub a: String,
    full: String,
    fin: bool,
}
impl Btw {
    pub fn done(&self) -> bool {
        self.fin && self.a.len() >= self.full.len()
    }
}

pub struct App {
    pub bs: Vec<B>,
    pub inp: String,
    pub cur: usize,
    pub scr: usize,
    pub open: bool,
    q: VecDeque<Sp>,
    pub busy: bool,
    pub t: u64,
    wait: u32,
    pub model: usize,
    pub eff: usize,
    pub mode: usize,
    pub ctx: f32,
    pub h5: f32,
    pub d7: f32,
    pub cwd: String,
    pub br: String,
    pub btw: Option<Btw>,
    pub todo: Vec<(u8, String)>,
    pub stodo: bool,
    pub ask: Option<usize>,
    ti: Option<usize>,
    sb: Option<usize>,
    pub n: usize,
    pub hist: Vec<String>,
    hi: Option<usize>,
    pub quit: bool,
    pub toast: Option<(String, u64)>,
    pub t0: u64,
    pub sel: Option<usize>,
    pub ses: Vec<&'static str>,
    pub si: usize,
    pub cp: Cell<Option<(u16, u16)>>,
    pub smax: Cell<usize>,
    pub tc: u64,
    pub ts: u64,
    pub to: u64,
    pub tm: u64,
    tf: bool,
    pub mname: String,
    pub effs: String,
    lv: Option<live::Lv>,
    cx: Option<codex::Cx>,
    pub bgw: Option<&'static str>,
    pub work: bool,
    pub wk: Option<()>,
    pub rw: Option<(usize, Option<usize>)>,
    pub sug: Option<String>,
    pub ms: usize,
    pub rs: Option<(Vec<(String, String, String)>, usize)>,
    pub rq: String,
    pub bp: Option<(usize, Option<&'static str>)>,
    pub tp: Option<(usize, usize)>,
    pub pp: Option<usize>,
    le: u64,
}

fn tl(k: Tk, a: &str, sum: &str, body: Vec<Ln>) -> Tool {
    Tool { k, nm: k.nm().into(), a: a.into(), sum: sum.into(), body, st: St::Ok, t0: 0, t1: 0 }
}
fn p(s: &str) -> Ln {
    Ln::P(s.into())
}

pub const FEED_RS: &str = "use std::time::Instant;

pub struct Feed {
    pub off: f32,
    pub tgt: f32,
    items: Vec<Card>,
    last: Instant,
}

impl Feed {
    pub fn new(items: Vec<Card>) -> Self {
        Self { off: 0.0, tgt: 0.0, items, last: Instant::now() }
    }

    pub fn next(&mut self) {
        self.tgt = (self.tgt + 1.0).min(self.items.len() as f32 - 1.0);
    }

    pub fn tick(&mut self, dt: f32) {
        let k = 1.0 - (-14.0 * dt).exp();
        self.off += (self.tgt - self.off) * k;
        self.clamp();
    }

    fn clamp(&mut self) {
        self.off = self.off.clamp(0.0, self.items.len() as f32);
    }
}";

fn edit1() -> Vec<Ln> {
    vec![
        Ln::C(40, "".into()),
        Ln::D(41, "    pub fn tick(&mut self) {".into()),
        Ln::D(42, "        self.off += 0.18;".into()),
        Ln::A(41, "    pub fn tick(&mut self, dt: f32) {".into()),
        Ln::A(42, "        let k = 1.0 - (-14.0 * dt).exp();".into()),
        Ln::A(43, "        self.off += (self.tgt - self.off) * k;".into()),
        Ln::C(44, "        self.clamp();".into()),
        Ln::C(45, "    }".into()),
    ]
}

fn hist0() -> Vec<B> {
    vec![
        B::U("scrolling in the feed stutters at 144hz, fix it".into(), "14:02".into()),
        B::Th("The feed probably moves by a fixed step per frame. At 144 Hz that gets faster and uneven because poll does not tick exactly. Checking feed.rs and the main loop.".into()),
        B::Tl(Tool { t1: 9, ..tl(Tk::Read, "src/feed.rs", "212 lines", vec![]) }),
        B::Tl(Tool { t1: 4, ..tl(Tk::Grep, "fn tick", "3 matches", vec![p("src/feed.rs:41    pub fn tick(&mut self) {"), p("src/app.rs:88        self.feed.tick();"), p("src/swipe.rs:17    pub fn tick(&mut self) {")]) }),
        B::T("Found it. `Feed::tick` moves the offset by a fixed amount per frame:\n\n```rust\nself.off += 0.18;\n```\n\nAt 144 Hz that runs 2.4x faster than at 60 Hz, and since `poll` returns irregularly, it jumps. Switching to **delta time**.".into()),
        B::Tl(Tool { t1: 12, ..tl(Tk::Edit, "src/feed.rs", "+3 -2", edit1()) }),
        B::Tl(Tool { t1: 254, ..tl(Tk::Bash, "cargo build --release", "ok", vec![p("   Compiling rusttok v0.4.1 (/home/netter/projects/rusttok)"), p("    Finished `release` profile [optimized] target(s) in 8.42s")]) }),
        B::T("Done. Scrolling is now framerate independent and eases out instead of stopping hard. Try it with `cargo run --release`.".into()),
        B::Rc(rcfmt(12.4, 18_200, 0.08, 0.0), 0),
    ]
}

fn script(n: usize) -> Vec<Sp> {
    match n % 3 {
        0 => vec![
            Sp::Th("The snap at the end of a swipe still uses the old linear motion. Reusing the scroll curve so both feel the same.".into()),
            Sp::Tx("Updating the snap in `swipe.rs`.".into()),
            Sp::Tl(tl(Tk::Read, "src/swipe.rs", "96 lines", vec![]), 22),
            Sp::Tl(tl(Tk::Edit, "src/swipe.rs", "+4 -3", vec![
                Ln::C(16, "".into()),
                Ln::D(17, "    pub fn tick(&mut self) {".into()),
                Ln::D(18, "        self.x += self.v;".into()),
                Ln::D(19, "        self.v *= 0.9;".into()),
                Ln::A(17, "    pub fn tick(&mut self, dt: f32) {".into()),
                Ln::A(18, "        let k = 1.0 - (-14.0 * dt).exp();".into()),
                Ln::A(19, "        self.x += (self.snap() - self.x) * k;".into()),
                Ln::A(20, "        self.v = 0.0;".into()),
                Ln::C(21, "    }".into()),
            ]), 34),
            Sp::Tl(Tool { st: St::Err, ..tl(Tk::Bash, "cargo test", "1 failed", vec![p("running 24 tests"), p("test swipe::snap_back ... FAILED"), p("  assertion failed: x == 0.0  (left: 0.004)"), p("test result: FAILED. 23 passed; 1 failed")]) }, 60),
            Sp::Tx("A test expects the snap to land exactly on 0. The curve only approaches it, so I add a threshold at the end.".into()),
            Sp::Tl(tl(Tk::Edit, "src/swipe.rs", "+3 -0", vec![
                Ln::C(20, "        self.v = 0.0;".into()),
                Ln::A(21, "        if (self.snap() - self.x).abs() < 0.01 {".into()),
                Ln::A(22, "            self.x = self.snap();".into()),
                Ln::A(23, "        }".into()),
            ]), 26),
            Sp::Tl(tl(Tk::Bash, "cargo test", "24 passed", vec![p("running 24 tests"), p("test feed::smooth ... ok"), p("test swipe::snap_back ... ok"), p("test result: ok. 24 passed; 0 failed; finished in 0.31s")]), 70),
            Sp::Todo(vec![(2, "feed.rs to delta time".into()), (2, "check build".into()), (2, "easing for swipe snap".into())]),
            Sp::Tx("Done. The snap now eases out with the same curve as scrolling, and all 24 tests pass.".into()),
            Sp::Rc(rcfmt(19.6, 31_500, 0.14, 1.0)),
        ],
        1 => vec![
            Sp::Th("Committing both files, asking before the push since it leaves this machine.".into()),
            Sp::Tl(tl(Tk::Bash, "git add -A && git commit -m \"smooth scroll + snap\"", "1 commit", vec![p("[main 3f9c2e1] smooth scroll + snap"), p(" 2 files changed, 7 insertions(+), 5 deletions(-)")]), 26),
            Sp::Ask("git push origin main".into()),
        ],
        _ => vec![
            Sp::Th("There are several ways to make scrolling smoother. A subagent checks the render path while I look for approaches.".into()),
            Sp::Tl(tl(Tk::Task, "explore: where is the feed rendered", "4 files", vec![p("read src/ui.rs"), p("read src/feed.rs"), p("grep \"render_card\"  ->  2 matches"), p("read src/card.rs"), p("result: render in ui.rs:120, rows are whole cells")]), 60),
            Sp::Tl(tl(Tk::Web, "ratatui smooth scrolling sub-cell", "5 results", vec![p("ratatui.rs  ·  Rendering with half blocks"), p("github.com  ·  tachyonfx motion effects"), p("blog  ·  frame pacing in terminal apps")]), 30),
            Sp::Tx("Three ways to make scrolling even smoother:\n\n- **Sub-row rendering** with half blocks, twice as fine steps\n- **Prediction** of the next frame from swipe velocity\n- **Frame pacing** tied to the monitor refresh rate\n\nI would start with the first one, it makes the biggest visible difference.".into()),
            Sp::Rc(rcfmt(9.2, 22_700, 0.09, 0.0)),
        ],
    }
}

const BTW: &str = "`dt` is the time since the last frame in seconds. About 0.0069 at 144 Hz and 0.0167 at 60 Hz, so motion per second stays the same no matter how often it draws.";

fn take(s: &mut String, n: usize) -> String {
    let i = s.char_indices().nth(n).map(|x| x.0).unwrap_or(s.len());
    let r = s[..i].to_string();
    s.drain(..i);
    r
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        App {
            bs: hist0(),
            inp: String::new(),
            cur: 0,
            scr: 0,
            open: false,
            q: VecDeque::new(),
            busy: false,
            t: 0,
            wait: 0,
            model: 0,
            eff: 2,
            mode: 0,
            ctx: 34.0,
            h5: 12.0,
            d7: 4.0,
            cwd: "~/projects/rusttok".into(),
            br: "main".into(),
            btw: None,
            todo: vec![(2, "feed.rs to delta time".into()), (2, "check build".into()), (0, "easing for swipe snap".into())],
            stodo: false,
            ask: None,
            ti: None,
            sb: None,
            n: 0,
            hist: vec![],
            hi: None,
            quit: false,
            toast: None,
            t0: 0,
            sel: None,
            ses: vec!["rusttok", "wargame", "hivemind"],
            si: 0,
            cp: Cell::new(None),
            smax: Cell::new(usize::MAX),
            tc: 0,
            ts: 0,
            to: 0,
            tm: 0,
            mname: MODELS[0].into(),
            effs: EFF[2].into(),
            lv: None,
            cx: None,
            bgw: None,
            work: false,
            wk: None,
            rw: None,
            sug: None,
            ms: 0,
            rs: None,
            rq: String::new(),
            bp: None,
            tp: None,
            pp: None,
            le: 0,
            tf: false,
        }
    }

    pub fn clock(&self) -> String {
        let m = 2 + 60 * 14 + (self.t / 1800) as u32;
        format!("{:02}:{:02}", m / 60, m % 60)
    }

    pub fn secs(&self, t: &Tool) -> f32 {
        let e = if t.st == St::Run { self.t } else { t.t1.max(t.t0) };
        (e - t.t0) as f32 * 0.033
    }

    pub fn turn_secs(&self) -> f32 {
        (self.t - self.t0) as f32 * 0.033
    }

    pub fn streaming(&self) -> Option<usize> {
        self.sb
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), self.t + 60));
    }

    pub fn step(&mut self) {
        self.t += 1;
        if let Some(b) = &mut self.btw {
            if !b.done() {
                let mut r = b.full[b.a.len()..].to_string();
                let c = take(&mut r, 3);
                b.a.push_str(&c);
            }
        }
        if let Some((_, t)) = &self.toast {
            if self.t > *t {
                self.toast = None;
            }
        }
        if self.lv.is_some() {
            self.pump();
            return;
        }
        if self.cx.is_some() {
            self.cpump();
            return;
        }
        if self.ask.is_some() {
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        let Some(sp) = self.q.pop_front() else {
            self.busy = false;
            return;
        };
        match sp {
            Sp::Th(mut s) => {
                let c = take(&mut s, 5);
                self.stream(true, c);
                if s.is_empty() {
                    self.sb = None;
                    self.wait = 8;
                } else {
                    self.q.push_front(Sp::Th(s));
                }
            }
            Sp::Tx(mut s) => {
                let c = take(&mut s, 3);
                self.stream(false, c);
                if s.is_empty() {
                    self.sb = None;
                    self.wait = 5;
                } else {
                    self.q.push_front(Sp::Tx(s));
                }
            }
            Sp::Tl(mut t, d) => {
                self.tf = t.st == St::Err;
                t.t0 = self.t;
                t.st = St::Run;
                self.bs.push(B::Tl(t));
                self.ti = Some(self.bs.len() - 1);
                self.sel = self.ti;
                self.wait = d;
                self.q.push_front(Sp::Fin);
            }
            Sp::Fin => {
                if let Some(i) = self.ti.take() {
                    if let B::Tl(t) = &mut self.bs[i] {
                        t.st = if self.tf { St::Err } else { St::Ok };
                        t.t1 = self.t;
                    }
                }
                self.tf = false;
                self.wait = 6;
                self.ctx = (self.ctx + 1.5).min(99.0);
            }
            Sp::Ask(c) => {
                self.bs.push(B::Ask(Ask { cmd: c, ans: None }));
                self.ask = Some(self.bs.len() - 1);
            }
            Sp::Todo(v) => self.todo = v,
            Sp::Rc(x) => {
                self.bs.push(B::Rc(x, self.t.max(1)));
                if self.q.is_empty() && self.inp.is_empty() {
                    self.sug = Some(["commit and push this", "can it get smoother?", "run the tests again"][self.n % 3].into());
                }
            }
        }
    }

    fn stream(&mut self, th: bool, c: String) {
        match self.sb {
            Some(i) => {
                if let B::Th(s) | B::T(s) = &mut self.bs[i] {
                    s.push_str(&c)
                }
            }
            None => {
                self.bs.push(if th { B::Th(c) } else { B::T(c) });
                self.sb = Some(self.bs.len() - 1);
            }
        }
        self.ctx = (self.ctx + 0.02).min(99.0);
        self.h5 = (self.h5 + 0.005).min(99.0);
    }

    pub fn answer(&mut self, c: char) {
        if self.lv.is_some() {
            self.lanswer(c);
            return;
        }
        let Some(i) = self.ask.take() else { return };
        if let B::Ask(a) = &mut self.bs[i] {
            a.ans = Some(c);
        }
        if c == 'n' {
            self.q.push_front(Sp::Rc(rcfmt(4.1, 9_800, 0.03, 0.0)));
            self.q.push_front(Sp::Tx("Okay, not pushed. The commit stays local on `main`.".into()));
        } else {
            self.q.push_front(Sp::Rc(rcfmt(6.8, 12_400, 0.04, 0.0)));
            self.q.push_front(Sp::Tx("Pushed to `origin/main`.".into()));
            self.q.push_front(Sp::Tl(tl(Tk::Bash, "git push origin main", "pushed", vec![p("To github.com:netter/rusttok.git"), p("   a81d0c4..3f9c2e1  main -> main")]), 40));
        }
    }

    pub fn stop(&mut self) {
        if self.cx.is_some() {
            self.cstop();
            return;
        }
        if self.lv.is_some() {
            self.lstop();
            return;
        }
        self.q.clear();
        self.wait = 0;
        if let Some(i) = self.ti.take() {
            if let B::Tl(t) = &mut self.bs[i] {
                t.st = St::Err;
                t.t1 = self.t;
                t.sum = "interrupted".into();
            }
        }
        if let Some(i) = self.ask.take() {
            if let B::Ask(a) = &mut self.bs[i] {
                a.ans = Some('n');
            }
        }
        self.sb = None;
        self.busy = false;
        self.bs.push(B::Sys("interrupted".into()));
    }

    pub fn submit(&mut self) {
        let s = self.inp.trim().to_string();
        if s.is_empty() {
            return;
        }
        self.inp.clear();
        self.cur = 0;
        self.scr = 0;
        self.sug = None;
        self.ts = self.t.max(1);
        self.hist.push(s.clone());
        self.hi = None;
        if let Some(k) = bg::canon(&s) {
            self.setbg(Some(k));
            return;
        }
        if let Some(r) = s.strip_prefix('/') {
            self.cmd(r);
            return;
        }
        if self.lv.is_some() {
            self.lsubmit(s);
            return;
        }
        if self.cx.is_some() {
            self.csubmit(s);
            return;
        }
        if !self.busy {
            self.t0 = self.t;
        }
        self.bs.push(B::U(s, self.clock()));
        self.busy = true;
        self.sb = None;
        let sc = script(self.n);
        self.n += 1;
        self.q.extend(sc);
    }

    pub fn setbg(&mut self, k: Option<&'static str>) {
        match k {
            Some(k) if !bg::exists(k) => {
                self.bs.push(B::Sys(format!("{k} is not installed  ·  {}", bg::hint(k))));
                return;
            }
            Some(k) => self.toast(format!("background  {k}")),
            None => self.toast("background  off"),
        }
        self.bgw = k;
        if self.lv.is_some() || self.cx.is_some() {
            let f = bgfile();
            if let Some(d) = std::path::Path::new(&f).parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&f, k.unwrap_or("off"));
        }
    }

    pub fn setdefmode(&mut self, i: usize) {
        if self.lv.is_some() || self.cx.is_some() {
            let f = modefile();
            if let Some(d) = std::path::Path::new(&f).parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&f, MODES[i]);
        }
        if self.mode != i {
            self.mode = i;
            self.tm = self.t.max(1);
            self.lmode();
        }
        self.toast(format!("default permission  {}", MODES[i]));
    }

    pub fn settheme(&mut self, i: usize) {
        pal::set(i);
        self.toast(format!("theme  {}", pal::THEMES[pal::cur()].name));
        if self.lv.is_some() || self.cx.is_some() {
            pal::save();
        }
    }

    pub fn cmd(&mut self, r: &str) {
        let (c0, a0) = r.split_once(' ').unwrap_or((r, ""));
        if c0 == "default-permission" {
            let a0 = a0.trim();
            if a0.is_empty() {
                self.pp = Some(defmode().unwrap_or(0));
            } else if let Some(i) = modeidx(a0) {
                self.setdefmode(i);
            } else {
                self.toast(format!("unknown mode  {a0}"));
            }
            return;
        }
        if c0 == "theme" {
            let a0 = a0.trim();
            if a0.is_empty() {
                self.tp = Some((pal::cur(), pal::cur()));
            } else if let Some(i) = pal::find(a0) {
                self.settheme(i);
            } else {
                self.toast(format!("unknown theme  {a0}"));
            }
            return;
        }
        if c0 == "background" || c0 == "bg" {
            let a0 = a0.trim();
            if a0.is_empty() {
                let v = bg::avail();
                let i = v.iter().position(|k| Some(*k) == self.bgw).unwrap_or(0);
                self.bp = Some((i, self.bgw));
            } else if a0 == "off" {
                self.setbg(None);
            } else if let Some(k) = bg::canon(a0) {
                self.setbg(Some(k));
            } else {
                self.toast(format!("unknown background  {a0}"));
            }
            return;
        }
        if self.lv.is_some() {
            self.lcmd(r);
            return;
        }
        if self.cx.is_some() {
            self.ccmd(r);
            return;
        }
        let (c, arg) = r.split_once(' ').unwrap_or((r, ""));
        match c {
            "btw" => {
                let q = if arg.is_empty() { "what is dt exactly?" } else { arg };
                self.btw = Some(Btw { q: q.into(), a: String::new(), full: BTW.into(), fin: true })
            }
            "clear" => {
                self.tc = self.t;
                self.bs.clear();
                self.ctx = 3.0;
                self.todo.clear();
                self.sel = None;
            }
            "model" => {
                self.model = (self.model + 1) % MODELS.len();
                self.toast(format!("model  {}", MODELS[self.model]))
            }
            "effort" => {
                self.eff = (self.eff + 1) % EFF.len();
                self.toast(format!("effort  {}", EFF[self.eff]))
            }
            "compact" => {
                let o = self.ctx;
                self.ctx = 9.0;
                self.bs.push(B::Sys(format!("compacted  ctx {:.0}% -> 9%", o)))
            }
            "context" => self.bs.push(B::Sys(format!("context  {:.0}% of 1M used  ·  system 8%  ·  tools 6%  ·  messages {:.0}%", self.ctx, self.ctx - 14.0))),
            "usage" => self.bs.push(B::Sys(format!("usage  5h {:.0}%  ·  7d {:.0}%  ·  opus 7d 2%", self.h5, self.d7))),
            "resume" => self.bs.push(B::Sys("resume  (mock) 12 older sessions in ~/projects/rusttok".into())),
            "help" => self.bs.push(B::Sys("enter send  shift+enter newline  esc stop  ctrl+o details  ctrl+t todos  shift+tab mode".into())),
            _ => self.bs.push(B::Sys(format!("unknown command  /{c}"))),
        }
    }

    fn bi(&self) -> usize {
        self.inp.char_indices().nth(self.cur).map(|x| x.0).unwrap_or(self.inp.len())
    }

    pub fn ins(&mut self, c: char) {
        self.sug = None;
        self.ms = 0;
        let i = self.bi();
        self.inp.insert(i, c);
        self.cur += 1;
    }

    fn nch(&self) -> usize {
        self.inp.chars().count()
    }

    pub fn rhits(&self) -> Vec<(String, String, String)> {
        let Some((v, _)) = &self.rs else { return vec![] };
        let q = self.rq.to_lowercase();
        v.iter().filter(|x| q.is_empty() || x.1.to_lowercase().contains(&q) || x.2.to_lowercase().contains(&q)).cloned().collect()
    }

    pub fn users(&self) -> Vec<usize> {
        self.bs.iter().enumerate().filter(|(_, b)| matches!(b, B::U(..))).map(|(i, _)| i).collect()
    }

    fn rwkey(&mut self, k: KeyEvent) {
        let Some((sel, opt)) = self.rw else { return };
        let us = self.users();
        match (k.code, opt) {
            (KeyCode::Esc, Some(_)) => self.rw = Some((sel, None)),
            (KeyCode::Esc, None) => self.rw = None,
            (KeyCode::Up, None) => self.rw = Some((sel.saturating_sub(1), None)),
            (KeyCode::Down, None) => self.rw = Some(((sel + 1).min(us.len().saturating_sub(1)), None)),
            (KeyCode::Enter, None) => self.rw = Some((sel, Some(0))),
            (KeyCode::Left, Some(o)) => self.rw = Some((sel, Some(o.saturating_sub(1)))),
            (KeyCode::Right | KeyCode::Tab, Some(o)) => self.rw = Some((sel, Some((o + 1).min(2)))),
            (KeyCode::Char(c @ '1'..='3'), Some(_)) => self.rw = Some((sel, Some(c as usize - '1' as usize))),
            (KeyCode::Enter, Some(o)) => {
                self.rw = None;
                if let Some(&i) = us.get(sel) {
                    self.rewind(i, o);
                }
            }
            _ => {}
        }
    }

    pub fn rewind(&mut self, i: usize, mode: usize) {
        if self.lv.is_some() {
            self.lrewind(i, mode);
            return;
        }
        if mode != 2 {
            if let Some(B::U(t, _)) = self.bs.get(i) {
                self.inp = t.clone();
                self.cur = self.inp.chars().count();
            }
            self.bs.truncate(i);
            self.q.clear();
            self.busy = false;
        }
        self.toast("rewound");
    }

    pub fn key(&mut self, k: KeyEvent) {
        let ct = k.modifiers.contains(Km::CONTROL);
        if self.rw.is_some() && !(ct && k.code == KeyCode::Char('c')) {
            self.rwkey(k);
            return;
        }
        if let Some(sel) = self.pp {
            if ct && k.code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            match k.code {
                KeyCode::Esc => self.pp = None,
                KeyCode::Up => self.pp = Some(sel.saturating_sub(1)),
                KeyCode::Down => self.pp = Some((sel + 1).min(MODES.len() - 1)),
                KeyCode::Enter => {
                    self.pp = None;
                    self.setdefmode(sel);
                }
                _ => {}
            }
            return;
        }
        if let Some((sel, prev)) = self.tp {
            if ct && k.code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            let n = pal::THEMES.len();
            match k.code {
                KeyCode::Esc => {
                    pal::set(prev);
                    self.tp = None;
                }
                KeyCode::Up | KeyCode::Down => {
                    let j = if k.code == KeyCode::Up { sel.saturating_sub(1) } else { (sel + 1).min(n - 1) };
                    self.tp = Some((j, prev));
                    pal::set(j);
                }
                KeyCode::Enter => {
                    self.tp = None;
                    self.settheme(sel);
                }
                _ => {}
            }
            return;
        }
        if let Some((sel, prev)) = self.bp {
            if ct && k.code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            let v = bg::avail();
            let pv = |i: usize| if v[i] == "off" { None } else { Some(v[i]) };
            match k.code {
                KeyCode::Esc => {
                    self.bgw = prev;
                    self.bp = None;
                }
                KeyCode::Up | KeyCode::Down => {
                    let n = if k.code == KeyCode::Up { sel.saturating_sub(1) } else { (sel + 1).min(v.len() - 1) };
                    self.bp = Some((n, prev));
                    self.bgw = pv(n);
                }
                KeyCode::Enter => {
                    self.bp = None;
                    self.setbg(pv(sel));
                }
                _ => {}
            }
            return;
        }
        if self.rs.is_some() && !(ct && k.code == KeyCode::Char('c')) {
            let hits = self.rhits();
            let Some((_, sel)) = &mut self.rs else { return };
            match k.code {
                KeyCode::Esc if !self.rq.is_empty() => {
                    self.rq.clear();
                    *sel = 0
                }
                KeyCode::Esc => self.rs = None,
                KeyCode::Up => *sel = sel.saturating_sub(1),
                KeyCode::Down => *sel = (*sel + 1).min(hits.len().saturating_sub(1)),
                KeyCode::Backspace => {
                    self.rq.pop();
                    *sel = 0
                }
                KeyCode::Char(c) => {
                    self.rq.push(c);
                    *sel = 0
                }
                KeyCode::Enter => {
                    let id = hits.get(*sel).map(|x| x.0.clone());
                    self.rs = None;
                    self.rq.clear();
                    if let Some(id) = id {
                        self.lresume(&id);
                    }
                }
                _ => {}
            }
            return;
        }
        match k.code {
            KeyCode::Char('c') if ct => self.quit = true,
            KeyCode::Char('d') if ct && self.inp.is_empty() => self.quit = true,
            KeyCode::Esc => {
                let dbl = self.t.saturating_sub(self.le) < 18;
                self.le = self.t;
                if self.btw.is_some() {
                    self.btw = None
                } else if self.busy {
                    self.stop()
                } else if !self.inp.is_empty() {
                    self.inp.clear();
                    self.cur = 0
                } else if dbl && !self.users().is_empty() {
                    self.rw = Some((self.users().len() - 1, None));
                    self.le = 0;
                }
            }
            KeyCode::Enter if k.modifiers.intersects(Km::SHIFT | Km::ALT) => self.ins('\n'),
            KeyCode::Enter => {
                if self.ask.is_some() && self.inp.is_empty() {
                    self.answer('y')
                } else {
                    self.submit()
                }
            }
            KeyCode::Char(c) if !ct && self.ask.is_some() && self.inp.is_empty() && matches!(c, 'y' | 'n' | 'a') => self.answer(c),
            KeyCode::Char('o') if ct => {
                self.open = !self.open;
                self.to = self.t
            }
            KeyCode::Char('t') if ct => self.stodo = !self.stodo,
            KeyCode::Char('a') if ct => self.cur = 0,
            KeyCode::Char('e') if ct => self.cur = self.nch(),
            KeyCode::Char('u') if ct => {
                let i = self.bi();
                self.inp.drain(..i);
                self.cur = 0
            }
            KeyCode::Char('w') if ct => {
                let cs: Vec<char> = self.inp.chars().collect();
                let mut j = self.cur;
                while j > 0 && cs[j - 1] == ' ' {
                    j -= 1
                }
                while j > 0 && cs[j - 1] != ' ' {
                    j -= 1
                }
                let s: String = cs[..j].iter().chain(cs[self.cur..].iter()).collect();
                self.inp = s;
                self.cur = j
            }
            KeyCode::BackTab => {
                self.mode = (self.mode + 1) % MODES.len();
                self.tm = self.t.max(1);
                self.lmode();
                self.toast(format!("mode  {}", MODES[self.mode]))
            }
            KeyCode::Char(c) if !ct => self.ins(c),
            KeyCode::Backspace => {
                self.ms = 0;
                if self.cur > 0 {
                    self.cur -= 1;
                    let i = self.bi();
                    self.inp.remove(i);
                }
            }
            KeyCode::Delete => {
                if self.cur < self.nch() {
                    let i = self.bi();
                    self.inp.remove(i);
                }
            }
            KeyCode::Left => self.cur = self.cur.saturating_sub(1),
            KeyCode::Right => self.cur = (self.cur + 1).min(self.nch()),
            KeyCode::Home => self.cur = 0,
            KeyCode::End => self.cur = self.nch(),
            KeyCode::Up if !self.inp.contains('\n') => {
                if self.hist.is_empty() {
                    return;
                }
                let i = match self.hi {
                    None => self.hist.len() - 1,
                    Some(i) => i.saturating_sub(1),
                };
                self.hi = Some(i);
                self.inp = self.hist[i].clone();
                self.cur = self.nch()
            }
            KeyCode::Down if !self.inp.contains('\n') => {
                if let Some(i) = self.hi {
                    if i + 1 < self.hist.len() {
                        self.hi = Some(i + 1);
                        self.inp = self.hist[i + 1].clone();
                    } else {
                        self.hi = None;
                        self.inp.clear();
                    }
                    self.cur = self.nch()
                }
            }
            KeyCode::PageUp => self.scr = (self.scr + 10).min(self.smax.get()),
            KeyCode::PageDown => self.scr = self.scr.saturating_sub(10),
            _ => {}
        }
    }

    pub fn tools(&self) -> Vec<usize> {
        self.bs.iter().enumerate().filter(|(_, b)| matches!(b, B::Tl(_))).map(|(i, _)| i).collect()
    }
}

pub fn wrap(s: &str, w: usize) -> Vec<String> {
    let mut o = vec![];
    for para in s.split('\n') {
        for l in wsegs(&[(para.to_string(), Style::default())], w) {
            o.push(l.iter().map(|x| x.content.to_string()).collect::<String>());
        }
    }
    o
}

fn words(s: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    let mut sp = None;
    for c in s.chars() {
        let isp = c == ' ';
        if sp == Some(isp) {
            v.last_mut().unwrap().push(c);
        } else {
            v.push(c.to_string());
            sp = Some(isp);
        }
    }
    v
}

pub fn wsegs(segs: &[(String, Style)], w: usize) -> Vec<Vec<Span<'static>>> {
    let w = w.max(4);
    let mut ls = vec![];
    let mut cur: Vec<Span<'static>> = vec![];
    let mut cw = 0;
    for (t, st) in segs {
        for wd in words(t) {
            let ww = wd.width();
            if wd.starts_with(' ') {
                if cw == 0 {
                    continue;
                }
                if cw + ww > w {
                    ls.push(std::mem::take(&mut cur));
                    cw = 0;
                    continue;
                }
                cur.push(Span::styled(wd, *st));
                cw += ww;
                continue;
            }
            if cw + ww > w && cw > 0 {
                while let Some(l) = cur.last() {
                    if l.content.trim().is_empty() {
                        cur.pop();
                    } else {
                        break;
                    }
                }
                ls.push(std::mem::take(&mut cur));
                cw = 0;
            }
            if ww > w {
                let mut b = String::new();
                for c in wd.chars() {
                    let x = c.width().unwrap_or(0);
                    if cw + x > w {
                        cur.push(Span::styled(std::mem::take(&mut b), *st));
                        ls.push(std::mem::take(&mut cur));
                        cw = 0;
                    }
                    b.push(c);
                    cw += x;
                }
                cur.push(Span::styled(b, *st));
                continue;
            }
            cur.push(Span::styled(wd, *st));
            cw += ww;
        }
    }
    if !cur.is_empty() || ls.is_empty() {
        ls.push(cur);
    }
    ls
}

pub struct Md {
    pub t: Style,
    pub b: Style,
    pub c: Style,
    pub fe: Style,
    pub kw: Style,
    pub st: Style,
    pub cm: Style,
    pub nu: Style,
    pub bul: &'static str,
    pub bs: Style,
    pub fpre: &'static str,
    pub fps: Style,
}

fn inl(s: &str, base: Style, m: &Md) -> Vec<(String, Style)> {
    let mut v = vec![];
    let mut cur = String::new();
    let (mut code, mut bold) = (false, false);
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let sty = |c: bool, b: bool| if c { m.c } else if b { m.b } else { base };
    while i < cs.len() {
        if cs[i] == '`' {
            v.push((std::mem::take(&mut cur), sty(code, bold)));
            code = !code;
            i += 1;
            continue;
        }
        if !code && cs[i] == '*' && i + 1 < cs.len() && cs[i + 1] == '*' {
            v.push((std::mem::take(&mut cur), sty(code, bold)));
            bold = !bold;
            i += 2;
            continue;
        }
        cur.push(cs[i]);
        i += 1;
    }
    v.push((cur, sty(code, bold)));
    v.retain(|x| !x.0.is_empty());
    v
}

const KW: [&str; 16] = ["fn", "let", "mut", "pub", "self", "Self", "impl", "struct", "use", "for", "in", "if", "else", "match", "return", "as"];

pub fn hl(s: &str, m: &Md) -> Vec<Span<'static>> {
    let mut v = vec![];
    let (code, cmt) = match s.find("//") {
        Some(i) => (&s[..i], Some(&s[i..])),
        None => (s, None),
    };
    let mut w = String::new();
    let mut ins = false;
    let flush = |w: &mut String, v: &mut Vec<Span<'static>>| {
        if w.is_empty() {
            return;
        }
        let t = std::mem::take(w);
        let st = if KW.contains(&t.as_str()) {
            m.kw
        } else if t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            m.nu
        } else {
            m.fe
        };
        v.push(Span::styled(t, st));
    };
    for c in code.chars() {
        if ins {
            w.push(c);
            if c == '"' {
                v.push(Span::styled(std::mem::take(&mut w), m.st));
                ins = false;
            }
            continue;
        }
        if c == '"' {
            flush(&mut w, &mut v);
            ins = true;
            w.push(c);
            continue;
        }
        if c.is_alphanumeric() || c == '_' || (c == '.' && w.chars().next().is_some_and(|x| x.is_ascii_digit())) {
            w.push(c);
        } else {
            flush(&mut w, &mut v);
            v.push(Span::styled(c.to_string(), m.fe));
        }
    }
    if ins {
        v.push(Span::styled(w, m.st));
    } else {
        flush(&mut w, &mut v);
    }
    if let Some(c) = cmt {
        v.push(Span::styled(c.to_string(), m.cm));
    }
    v
}

pub fn md(s: &str, w: usize, m: &Md) -> Vec<Line<'static>> {
    let mut o = vec![];
    let mut inf = false;
    for raw in s.lines() {
        if raw.trim_start().starts_with("```") {
            inf = !inf;
            continue;
        }
        if inf {
            let mut sp = vec![Span::styled(m.fpre.to_string(), m.fps)];
            sp.extend(hl(raw, m));
            o.push(Line::from(sp));
            continue;
        }
        if raw.trim().is_empty() {
            o.push(Line::default());
            continue;
        }
        if raw.trim_start().starts_with('|') {
            let t = raw.trim();
            if t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                continue;
            }
            let cs: Vec<&str> = t.trim_matches('|').split('|').map(|x| x.trim()).collect();
            let mut sp = vec![];
            for (i, c) in cs.iter().enumerate() {
                if i > 0 {
                    sp.push(Span::styled("  ·  ", m.fps));
                }
                sp.extend(inl(c, if i == 0 { m.t } else { m.fps }, m).into_iter().map(|(x, y)| Span::styled(x, y)));
            }
            o.push(Line::from(sp));
            continue;
        }
        let (pre, body) = match raw.strip_prefix("- ") {
            Some(r) => (m.bul.to_string(), r),
            None => (String::new(), raw),
        };
        let pw = pre.width();
        let ind = " ".repeat(pw);
        for (i, l) in wsegs(&inl(body, m.t, m), w.saturating_sub(pw)).into_iter().enumerate() {
            let mut v = vec![Span::styled(if i == 0 { pre.clone() } else { ind.clone() }, m.bs)];
            v.extend(l);
            o.push(Line::from(v));
        }
    }
    let mut r: Vec<Line<'static>> = vec![];
    for l in o {
        let e = l.spans.iter().all(|x| x.content.trim().is_empty());
        if e && r.last().is_none_or(|p: &Line| p.spans.iter().all(|x| x.content.trim().is_empty())) {
            continue;
        }
        r.push(l);
    }
    while r.last().is_some_and(|l| l.spans.iter().all(|x| x.content.trim().is_empty())) {
        r.pop();
    }
    r
}

pub fn tail(v: Vec<Line<'static>>, h: usize, scr: &mut usize) -> Vec<Line<'static>> {
    let n = v.len();
    if n <= h {
        *scr = 0;
        return v;
    }
    *scr = (*scr).min(n - h);
    let s = n - h - *scr;
    v.into_iter().skip(s).take(h).collect()
}

pub fn inwrap(s: &str, cur: usize, w: usize) -> (Vec<String>, (usize, usize)) {
    let w = w.max(2);
    let mut ls = vec![String::new()];
    let (mut cx, mut cy) = (0, 0);
    let mut col = 0;
    for (i, c) in s.chars().enumerate() {
        if i == cur {
            cx = col;
            cy = ls.len() - 1;
        }
        if c == '\n' {
            ls.push(String::new());
            col = 0;
            continue;
        }
        let x = c.width().unwrap_or(0);
        if col + x > w {
            ls.push(String::new());
            col = 0;
            if i == cur {
                cx = 0;
                cy = ls.len() - 1;
            }
        }
        ls.last_mut().unwrap().push(c);
        col += x;
    }
    if cur >= s.chars().count() {
        if col >= w {
            ls.push(String::new());
            col = 0;
        }
        cx = col;
        cy = ls.len() - 1;
    }
    (ls, (cx, cy))
}

pub fn bar(p: f32, n: usize, on: &str, off: &str) -> (String, String) {
    let k = ((p / 100.0) * n as f32).round() as usize;
    let k = k.min(n);
    (on.repeat(k), off.repeat(n - k))
}

pub fn lr(l: Vec<Span<'static>>, r: Vec<Span<'static>>, w: usize) -> Line<'static> {
    let lw: usize = l.iter().map(|s| s.content.width()).sum();
    let rw: usize = r.iter().map(|s| s.content.width()).sum();
    let mut v = l;
    v.push(Span::raw(" ".repeat(w.saturating_sub(lw + rw).max(1))));
    v.extend(r);
    Line::from(v)
}

pub fn sw(v: &[Span]) -> usize {
    v.iter().map(|s| s.content.width()).sum()
}

pub fn setcur(f: &mut Frame, a: &App, x: u16, y: u16) {
    f.set_cursor_position((x, y));
    a.cp.set(Some((x, y)));
}

pub fn st(c: Color) -> Style {
    Style::new().fg(c)
}

pub trait V {
    fn draw(&mut self, f: &mut Frame, a: &App);
    fn key(&mut self, _k: &KeyEvent, _a: &mut App) -> bool {
        false
    }
    fn scen(&mut self, _s: &str, _a: &mut App) {}
}

pub fn run<T: V + Clone>(mut v: T) -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--dump" {
        return dump(&mut v, &args[2..]);
    }
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let _ = write!(out, "\x1b[22;0t");
    let _ = execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        let _ = restore();
        hook(p)
    }));
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    let r = lp(&mut term, &mut v);
    restore()?;
    if let Err(e) = &r {
        eprintln!("noname: {e}");
        std::process::exit(1);
    }
    r
}

fn lp<T: V>(term: &mut Terminal<CrosstermBackend<io::Stdout>>, v: &mut T) -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cx = args.iter().any(|x| x == "--codex") || std::env::var("NONAME_ENGINE").is_ok_and(|x| x == "codex");
    let args: Vec<String> = args.into_iter().filter(|x| x != "--codex").collect();
    let mut a = if std::env::var("NONAME_MOCK").is_ok() {
        App::new()
    } else if cx {
        App::codex(args)?
    } else {
        App::live(args)?
    };
    let mut lt = String::new();
    sys::on_close(live::killall);
    while !a.quit {
        a.cp.set(None);
        let cf = term.draw(|f| v.draw(f, &a))?;
        if let Ok(p) = std::env::var("NONAME_SHOT") {
            if p.contains("{}") {
                if a.t % 3 == 0 {
                    let _ = std::fs::write(p.replace("{}", &format!("{:05}", a.t / 3)), html(cf.buffer, a.cp.get()));
                }
            } else if a.t % 15 == 0 {
                let _ = std::fs::write(&p, html(cf.buffer, a.cp.get()));
            }
        }
        let tt = ttl(&a);
        if tt != lt {
            let _ = execute!(io::stdout(), ratatui::crossterm::terminal::SetTitle(&tt));
            lt = tt;
        }
        let dl = Instant::now() + Duration::from_millis(33);
        while event::poll(dl.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    if !v.key(&k, &mut a) {
                        a.key(k)
                    }
                }
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => a.scr = (a.scr + 3).min(a.smax.get()),
                    MouseEventKind::ScrollDown => a.scr = a.scr.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
            if a.quit {
                break;
            }
        }
        if let Ok(cy) = std::env::var("NONAME_BGCYCLE") {
            let v: Vec<&str> = cy.split(',').collect();
            if a.t % 30 == 0 && !v.is_empty() {
                a.bgw = bg::canon(v[(a.t / 30) as usize % v.len()]);
            }
        }
        a.step();
    }
    Ok(())
}

pub fn modefile() -> String {
    sys::cfgfile("permission")
}

pub fn modeidx(s: &str) -> Option<usize> {
    match s.trim().to_lowercase().as_str() {
        "default" | "ask" | "normal" => Some(0),
        "plan" => Some(1),
        "auto" | "acceptedits" | "accept-edits" => Some(2),
        "bypass" | "bypasspermissions" | "yolo" => Some(3),
        _ => None,
    }
}

pub fn defmode() -> Option<usize> {
    std::fs::read_to_string(modefile()).ok().and_then(|x| modeidx(&x))
}

pub fn bgfile() -> String {
    sys::cfgfile("background")
}

fn ttl(a: &App) -> String {
    let d = a.cwd.rsplit('/').next().unwrap_or("").to_string();
    let st = if a.ask.is_some() { " · waiting" } else if a.busy { " · working" } else { "" };
    format!("noname · {d}{st}")
}

fn restore() -> io::Result<()> {
    let mut o = io::stdout();
    let _ = execute!(o, PopKeyboardEnhancementFlags);
    let _ = write!(o, "\x1b[23;0t");
    disable_raw_mode()?;
    execute!(o, LeaveAlternateScreen, DisableMouseCapture, ratatui::crossterm::cursor::Show)?;
    Ok(())
}

fn setup<T: V>(v: &mut T, a: &mut App, s: &str) {
    if let Some((x, n)) = s.split_once('+') {
        setup(v, a, x);
        for _ in 0..n.parse::<u32>().unwrap_or(0) {
            a.step()
        }
        return;
    }
    match s {
        "empty" => a.cmd("clear"),
        "theme" => {
            a.inp = "/theme".into();
            a.submit();
        }
        "run" => {
            a.inp = "make the swipe snap that smooth too".into();
            a.submit();
            for _ in 0..150 {
                a.step()
            }
        }
        "ask" => {
            a.n = 1;
            a.inp = "commit and push this".into();
            a.submit();
            for _ in 0..1500 {
                a.step();
                if a.ask.is_some() {
                    break;
                }
            }
        }
        "btw" => {
            a.inp = "/btw what is dt exactly?".into();
            a.submit();
            for _ in 0..200 {
                a.step()
            }
        }
        "go" => {
            a.n = 2;
            a.inp = "can it get smoother?".into();
            a.submit();
        }
        "fail" => {
            a.inp = "make the swipe snap that smooth too".into();
            a.submit();
            for _ in 0..3000 {
                a.step();
                if a.bs.iter().any(|b| matches!(b, B::Tl(t) if t.st == St::Err)) {
                    break;
                }
            }
        }
        "open" => {
            a.open = true;
            a.to = a.t;
        }
        "type" => {
            a.inp = "can you give the snap\nsome easing and commit right after".into();
            a.cur = a.inp.chars().count();
        }
        "todo" => a.stodo = true,
        "done" => {
            a.n = 2;
            a.inp = "can it get smoother?".into();
            a.submit();
            for _ in 0..2500 {
                a.step();
                if !a.busy {
                    break;
                }
            }
        }
        x => v.scen(x, a),
    }
}

fn hex(c: Color) -> Option<String> {
    Some(match c {
        Color::Reset => return None,
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Black => "#22192f".into(),
        Color::Red => "#f38ba8".into(),
        Color::Green => "#a6e3a1".into(),
        Color::Blue => "#b497ff".into(),
        Color::Magenta => "#d0a9ff".into(),
        Color::Cyan => "#89dceb".into(),
        Color::Gray => "#d9d0ee".into(),
        Color::DarkGray => "#3a2b52".into(),
        Color::White => "#f7f2ff".into(),
        _ => "#e9defa".into(),
    })
}

fn css(c: &ratatui::buffer::Cell) -> String {
    let mut s = String::new();
    if let Some(h) = hex(c.fg) {
        s += &format!("color:{h};");
    }
    if let Some(h) = hex(c.bg) {
        s += &format!("background:{h};");
    }
    let m = c.modifier;
    if m.contains(Modifier::BOLD) {
        s += "font-weight:700;";
    }
    if m.contains(Modifier::DIM) {
        s += "opacity:.55;";
    }
    if m.contains(Modifier::ITALIC) {
        s += "font-style:italic;";
    }
    if m.contains(Modifier::UNDERLINED) {
        s += "text-decoration:underline;";
    }
    if m.contains(Modifier::CROSSED_OUT) {
        s += "text-decoration:line-through;";
    }
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn html(b: &Buffer, cur: Option<(u16, u16)>) -> String {
    let a = b.area;
    let pb = std::env::var("NONAME_SHOT_BG").ok();
    let mut s = String::from("<!doctype html><html><head><meta charset=utf-8><style>body{margin:0;background:#0d0a14}#bg{position:relative;display:inline-block;padding:14px 18px;background:radial-gradient(circle at 20% 30%,#5a3d8a 0,transparent 45%),radial-gradient(circle at 80% 70%,#2a4a7a 0,transparent 50%),linear-gradient(135deg,#2b1f45,#1b2440 50%,#3a2040)}#bg:before{content:'';position:absolute;inset:0;background:rgba(26,20,38,.7)}#t{position:relative;font:15px/19px 'CaskaydiaCove Nerd Font Mono',monospace;white-space:pre;color:#e9defa}.k{background:#b497ff;color:#1a1426}</style></head><body><div id=bg><div id=t>");
    s += &format!("<style>.k{{background:{};color:{}}}#t{{color:{}}}</style>", hex(pal::LI()).unwrap_or_default(), hex(pal::BASE()).unwrap_or_default(), hex(pal::FG()).unwrap_or_default());
    if let Some(c) = &pb {
        s += &format!("<style>body,#bg{{background:{c} !important}}#bg:before{{display:none}}</style>");
    }
    for y in 0..a.height {
        let mut x = 0;
        let mut run = String::new();
        let mut rs = String::new();
        while x < a.width {
            let c = &b[(x, y)];
            let k = cur == Some((x, y));
            let cs = if k { "K".to_string() } else { css(c) };
            if cs != rs && !run.is_empty() {
                s += &span(&rs, &run);
                run.clear();
            }
            rs = cs;
            let sym = c.symbol();
            run += &esc(if sym.is_empty() { "" } else { sym });
            x += 1;
        }
        if !run.is_empty() {
            s += &span(&rs, &run);
        }
        s.push('\n');
    }
    s += "</div></div></body></html>";
    s
}

fn span(cs: &str, t: &str) -> String {
    if cs == "K" {
        format!("<span class=k>{t}</span>")
    } else if cs.is_empty() {
        t.to_string()
    } else {
        format!("<span style=\"{cs}\">{t}</span>")
    }
}

fn dump<T: V + Clone>(v0: &mut T, args: &[String]) -> io::Result<()> {
    let dir = args.first().cloned().unwrap_or_else(|| ".".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "v".into());
    let sizes: Vec<(u16, u16)> = vec![(270, 50), (104, 32)];
    let mut sc: Vec<String> = ["idle", "empty", "run", "ask", "btw", "open", "type", "todo", "done"].iter().map(|s| s.to_string()).collect();
    sc.extend(args.iter().skip(2).cloned());
    for (w, h) in sizes {
        for s in &sc {
            let mut a = App::new();
            let mut vv = v0.clone();
            let v = &mut vv;
            setup(v, &mut a, s);
            let mut term = Terminal::new(TestBackend::new(w, h))?;
            a.cp.set(None);
            term.draw(|f| v.draw(f, &a))?;
            let b = term.backend().buffer().clone();
            let mut txt = String::new();
            for y in 0..h {
                for x in 0..w {
                    txt += b[(x, y)].symbol();
                }
                txt += "\n";
            }
            let base = format!("{dir}/{name}-{s}-{w}");
            std::fs::write(format!("{base}.txt"), txt)?;
            std::fs::write(format!("{base}.html"), html(&b, a.cp.get()))?;
        }
    }
    Ok(())
}

const NZ: &[u8] = b"0123456789abcdefABCDEF$%@#";

pub fn rcfmt(secs: f32, tok: u64, usd: f64, dw: f32) -> String {
    let t = if tok >= 1000 { format!("{:.1}k", tok as f64 / 1000.0) } else { tok.to_string() };
    let mut s = format!("{secs:.1}s  ·  {t} tok  ·  ${usd:.2}");
    if dw >= 0.5 {
        s += &format!("  ·  +{dw:.0}% week");
    }
    s
}

pub fn pulse(t: u64, per: f32) -> f32 {
    ((t as f32 / per * std::f32::consts::TAU).sin() + 1.0) / 2.0
}

pub fn hsh(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

pub fn nz(t: u64, i: u64) -> char {
    NZ[(hsh(t / 2, i) % NZ.len() as u64) as usize] as char
}

fn rgb(c: Color) -> (f32, f32, f32) {
    match c {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => (233.0, 222.0, 250.0),
    }
}

pub fn grad(cs: &[Color], f: f32) -> Color {
    let f = f.clamp(0.0, 1.0) * (cs.len() - 1) as f32;
    let i = (f.floor() as usize).min(cs.len() - 2);
    let k = f - i as f32;
    let (a, b) = (rgb(cs[i]), rgb(cs[i + 1]));
    let m = |x: f32, y: f32| (x + (y - x) * k) as u8;
    Color::Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

fn wave(i: usize, t: u64) -> f32 {
    let f = (i as f32 * 0.13 + t as f32 * 0.06) % 2.0;
    if f > 1.0 { 2.0 - f } else { f }
}

pub fn noise(n: usize, t: u64, seed: u64, cs: &[Color]) -> Vec<Span<'static>> {
    (0..n).map(|i| Span::styled(nz(t, seed * 131 + i as u64).to_string(), Style::new().fg(grad(cs, wave(i, t))))).collect()
}

pub fn decode(s: &str, el: u64, t: u64, seed: u64, done: Style, cs: &[Color]) -> Vec<Span<'static>> {
    let mut v: Vec<Span<'static>> = vec![];
    let mut run = String::new();
    for (i, c) in s.chars().enumerate() {
        if c == ' ' || el >= 2 + i as u64 {
            run.push(c);
            continue;
        }
        if !run.is_empty() {
            v.push(Span::styled(std::mem::take(&mut run), done));
        }
        v.push(Span::styled(nz(t, seed * 131 + i as u64).to_string(), Style::new().fg(grad(cs, wave(i, t)))));
    }
    if !run.is_empty() {
        v.push(Span::styled(run, done));
    }
    v
}
