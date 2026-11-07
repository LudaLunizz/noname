use std::collections::HashMap;
use std::io::{self, BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

use serde_json::{Value, json};

use super::*;

pub struct Cx {
    tid: Option<String>,
    ch: Option<Child>,
    rx: Option<Receiver<Value>>,
    args: Vec<String>,
    im: HashMap<String, usize>,
    last: bool,
    t0: u64,
}

impl Drop for Cx {
    fn drop(&mut self) {
        if let Some(c) = &mut self.ch {
            live::reap_pub(c);
        }
    }
}

fn sv(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn one(s: &str, n: usize) -> String {
    let l = s.lines().next().unwrap_or("").trim();
    if l.chars().count() > n { l.chars().take(n).collect::<String>() + "…" } else { l.to_string() }
}

fn unwrap(c: &str) -> String {
    for p in ["bash -lc ", "/bin/bash -lc ", "bash -c ", "sh -c ", "zsh -lc ", "powershell.exe -Command ", "powershell -Command ", "pwsh -Command ", "pwsh.exe -Command ", "cmd /c ", "cmd.exe /c "] {
        if let Some(r) = c.strip_prefix(p) {
            let r = r.trim();
            if r.len() >= 2 && ((r.starts_with('"') && r.ends_with('"')) || (r.starts_with('\'') && r.ends_with('\''))) {
                return r[1..r.len() - 1].replace("\\\"", "\"");
            }
            return r.to_string();
        }
    }
    c.to_string()
}

fn cfg_model() -> String {
    let h = std::env::var("CODEX_HOME").unwrap_or_else(|_| format!("{}/.codex", crate::sys::home()));
    std::fs::read_to_string(format!("{h}/config.toml"))
        .ok()
        .and_then(|f| f.lines().find(|l| l.trim_start().starts_with("model ") || l.trim_start().starts_with("model=")).map(|l| l.split('=').nth(1).unwrap_or("").trim().trim_matches('"').to_string()))
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "codex".into())
}

impl App {
    pub fn codex(args: Vec<String>) -> io::Result<App> {
        let mut a = App::new();
        a.bs.clear();
        a.todo.clear();
        a.ctx = -1.0;
        a.h5 = -1.0;
        a.d7 = -1.0;
        a.cwd = {
            let c = std::env::current_dir()?.to_string_lossy().to_string();
            let h = crate::sys::home();
            if !h.is_empty() && c.starts_with(&h) { format!("~{}", &c[h.len()..]) } else { c }
        };
        a.br = Command::new("git").args(["branch", "--show-current"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let mut rest = vec![];
        let mut last = false;
        let mut tid = None;
        let mut it = args.into_iter();
        while let Some(x) = it.next() {
            match x.as_str() {
                "-c" | "--continue" => last = true,
                "-r" | "--resume" => tid = it.next(),
                "--model" | "-m" => {
                    if let Some(m) = it.next() {
                        rest.extend(["-m".to_string(), m]);
                    }
                }
                _ => rest.push(x),
            }
        }
        a.mname = rest.iter().position(|x| x == "-m").and_then(|i| rest.get(i + 1)).cloned().unwrap_or_else(cfg_model);
        a.effs = String::new();
        a.bgw = std::fs::read_to_string(crate::bgfile()).ok().and_then(|x| crate::bg::canon(&x)).filter(|k| crate::bg::exists(k));
        a.mode = crate::defmode().unwrap_or(0);
        a.cx = Some(Cx { tid, ch: None, rx: None, args: rest, im: HashMap::new(), last, t0: 0 });
        Ok(a)
    }

    pub fn is_codex(&self) -> bool {
        self.cx.is_some()
    }

    pub(crate) fn csubmit(&mut self, s: String) {
        if self.busy {
            self.toast("codex is busy");
            return;
        }
        self.t0 = self.t;
        self.bs.push(B::U(s.clone(), self.clock()));
        self.busy = true;
        let mode = self.mode;
        let t = self.t;
        let Some(c) = &mut self.cx else { return };
        let mut cmd = Command::new(crate::sys::bin("codex"));
        cmd.arg("exec");
        let resume = c.tid.is_some() || c.last;
        if resume {
            cmd.arg("resume");
        }
        cmd.args(["--json", "--skip-git-repo-check"]);
        match mode {
            1 => {
                cmd.args(["-s", "read-only"]);
            }
            3 => {
                cmd.arg("--dangerously-bypass-approvals-and-sandbox");
            }
            _ => {
                cmd.args(["-s", "workspace-write"]);
            }
        }
        cmd.args(&c.args);
        match (&c.tid, c.last) {
            (Some(id), _) => {
                cmd.arg(id);
            }
            (None, true) => {
                cmd.arg("--last");
            }
            _ => {}
        }
        cmd.arg(&s);
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        live::guard_pub(&mut cmd);
        let Ok(mut ch) = cmd.spawn() else {
            self.busy = false;
            self.bs.push(B::Sys("codex not found".into()));
            return;
        };
        live::track_pid(ch.id() as i32);
        let out = ch.stdout.take().unwrap();
        let (tx, rx) = channel();
        thread::spawn(move || {
            for l in BufReader::new(out).lines() {
                let Ok(l) = l else { break };
                if let Ok(v) = serde_json::from_str::<Value>(&l) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
            let _ = tx.send(json!({"type": "_eof"}));
        });
        c.ch = Some(ch);
        c.rx = Some(rx);
        c.last = false;
        c.t0 = t;
        c.im.clear();
    }

    pub(crate) fn cstop(&mut self) {
        if let Some(c) = &mut self.cx {
            if let Some(mut ch) = c.ch.take() {
                live::untrack_pid(ch.id() as i32);
                live::reap_pub(&mut ch);
            }
            c.rx = None;
        }
        for b in self.bs.iter_mut() {
            if let B::Tl(t) = b {
                if t.st == St::Run {
                    t.st = St::Err;
                    t.t1 = self.t;
                    t.sum = "interrupted".into();
                }
            }
        }
        self.busy = false;
        self.sb = None;
        self.bs.push(B::Sys("interrupted".into()));
    }

    pub(crate) fn ccmd(&mut self, r: &str) {
        let (c, arg) = r.split_once(' ').unwrap_or((r, ""));
        match c {
            "clear" | "new" => {
                if let Some(x) = &mut self.cx {
                    x.tid = None;
                    x.last = false;
                }
                self.bs.clear();
                self.todo.clear();
                self.tc = self.t;
            }
            "resume" if !arg.trim().is_empty() => {
                if let Some(x) = &mut self.cx {
                    x.tid = Some(arg.trim().to_string());
                }
                self.toast("next message resumes that session");
            }
            "quit" | "exit" => self.quit = true,
            _ => self.toast(format!("/{c} is not available with codex")),
        }
    }

    pub(crate) fn cpump(&mut self) {
        let evs: Vec<Value> = match self.cx.as_ref().and_then(|c| c.rx.as_ref()) {
            Some(r) => r.try_iter().collect(),
            None => return,
        };
        for e in evs {
            self.cev(e);
        }
    }

    fn cev(&mut self, e: Value) {
        match e["type"].as_str().unwrap_or("") {
            "thread.started" => {
                let id = sv(&e, "thread_id");
                if let Some(c) = &mut self.cx {
                    if c.tid.is_none() && !id.is_empty() {
                        c.tid = Some(id);
                    }
                }
            }
            "item.started" | "item.updated" | "item.completed" => {
                let done = e["type"] == "item.completed";
                self.citem(&e["item"], done);
            }
            "turn.completed" => {
                let u = &e["usage"];
                let tok = ["input_tokens", "cached_input_tokens", "output_tokens"].iter().map(|k| u[*k].as_u64().unwrap_or(0)).sum::<u64>();
                let t0 = self.cx.as_ref().map(|c| c.t0).unwrap_or(self.t);
                let secs = (self.t - t0) as f32 * 0.033;
                let tk = if tok >= 1000 { format!("{:.1}k", tok as f64 / 1000.0) } else { tok.to_string() };
                self.bs.push(B::Rc(format!("{secs:.1}s  ·  {tk} tok"), self.t.max(1)));
            }
            "turn.failed" | "error" => {
                let m = e["error"]["message"].as_str().map(|x| x.to_string()).unwrap_or_else(|| sv(&e, "message"));
                if !m.is_empty() {
                    self.bs.push(B::T(format!("**codex:** {}", one(&m, 200))));
                }
            }
            "_eof" => {
                self.busy = false;
                self.sb = None;
                for b in self.bs.iter_mut() {
                    if let B::Tl(t) = b {
                        if t.st == St::Run {
                            t.st = St::Ok;
                            t.t1 = self.t;
                        }
                    }
                }
                if let Some(c) = &mut self.cx {
                    if let Some(mut ch) = c.ch.take() {
                        live::untrack_pid(ch.id() as i32);
                        let _ = ch.wait();
                    }
                    c.rx = None;
                }
            }
            _ => {}
        }
    }

    fn citem(&mut self, it: &Value, done: bool) {
        let id = sv(it, "id");
        let ty = sv(it, "type");
        let ex = self.cx.as_ref().and_then(|c| c.im.get(&id).copied());
        let now = self.t.max(1);
        match ty.as_str() {
            "agent_message" | "reasoning" => {
                let txt = sv(it, "text");
                let b = if ty == "reasoning" { B::Th(txt.replace("**", "")) } else { B::T(txt) };
                match ex {
                    Some(i) => self.bs[i] = b,
                    None => {
                        self.bs.push(b);
                        let i = self.bs.len() - 1;
                        if let Some(c) = &mut self.cx {
                            c.im.insert(id, i);
                        }
                    }
                }
            }
            "todo_list" => {
                if let Some(a) = it["items"].as_array() {
                    self.todo = a.iter().map(|x| (if x["completed"] == true { 2 } else { 0 }, sv(x, "text"))).collect();
                }
            }
            "command_execution" | "file_change" | "mcp_tool_call" | "web_search" | "collab_tool_call" => {
                let (k, nm, a) = match ty.as_str() {
                    "command_execution" => (Tk::Bash, "bash".to_string(), one(&unwrap(&sv(it, "command")), 140)),
                    "file_change" => {
                        let ch = it["changes"].as_array().cloned().unwrap_or_default();
                        let p: Vec<String> = ch.iter().map(|c| sv(c, "path")).collect();
                        let p = p.iter().map(|x| x.rsplit('/').next().unwrap_or(x).to_string()).collect::<Vec<_>>().join(", ");
                        (Tk::Edit, "edit".to_string(), p)
                    }
                    "mcp_tool_call" => (Tk::Oth, sv(it, "server"), sv(it, "tool")),
                    "web_search" => (Tk::Web, "web".to_string(), sv(it, "query")),
                    _ => (Tk::Task, "agent".to_string(), one(&sv(it, "prompt"), 120)),
                };
                let st = match sv(it, "status").as_str() {
                    "failed" | "declined" => St::Err,
                    _ if done => St::Ok,
                    _ => St::Run,
                };
                let out = sv(it, "aggregated_output");
                let ls: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
                let sum = match (ty.as_str(), st) {
                    ("command_execution", St::Err) => format!("exit {}", it["exit_code"].as_i64().unwrap_or(1)),
                    ("command_execution", St::Ok) => ls.last().map(|l| one(l, 48)).unwrap_or_else(|| "ok".into()),
                    ("file_change", _) => {
                        let ch = it["changes"].as_array().map(|x| x.len()).unwrap_or(0);
                        format!("{ch} files")
                    }
                    (_, St::Err) => "failed".into(),
                    (_, St::Ok) => "done".into(),
                    _ => String::new(),
                };
                let body: Vec<Ln> = ls.iter().rev().take(12).rev().map(|l| Ln::P(one(l, 200))).collect();
                match ex {
                    Some(i) => {
                        if let Some(B::Tl(t)) = self.bs.get_mut(i) {
                            t.st = st;
                            t.sum = sum;
                            t.body = body;
                            if st != St::Run {
                                t.t1 = now;
                            }
                        }
                    }
                    None => {
                        self.bs.push(B::Tl(Tool { k, nm, a, sum, body, st, t0: now, t1: if st == St::Run { 0 } else { now } }));
                        let i = self.bs.len() - 1;
                        self.sel = Some(i);
                        if let Some(c) = &mut self.cx {
                            c.im.insert(id, i);
                        }
                    }
                }
            }
            "error" => self.bs.push(B::Sys(one(&sv(it, "message"), 200))),
            _ => {}
        }
    }
}
