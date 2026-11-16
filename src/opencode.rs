use std::process::Command;

use serde_json::Value;

use super::codex::{Cx, one, sv};
use super::*;

/// The model from the opencode config (`"model": "provider/model"`), or "opencode".
pub fn cfg_model() -> String {
    let h = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", crate::sys::home()));
    for f in ["opencode.jsonc", "opencode.json"] {
        if let Ok(t) = std::fs::read_to_string(format!("{h}/opencode/{f}")) {
            // top level keys sit at two spaces; nested ones are indented deeper
            for l in t.lines() {
                if let Some(r) = l.strip_prefix("  \"model\":") {
                    let m = r.trim().trim_end_matches(',').trim().trim_matches('"');
                    if !m.is_empty() {
                        return m.to_string();
                    }
                }
            }
        }
    }
    "opencode".into()
}

/// One `opencode run` per message, like the Codex engine runs one `codex exec`.
pub fn build(c: &Cx, mode: usize, imgs: &[(&'static str, Vec<u8>)], t: u64, x: &str) -> Command {
    let mut cmd = Command::new(crate::sys::bin("opencode"));
    cmd.args(["run", "--format", "json", "--thinking"]);
    match mode {
        1 => {
            cmd.args(["--agent", "plan"]);
        }
        // opencode run cannot ask: without --auto every permission request is rejected
        2 | 3 => {
            cmd.arg("--auto");
        }
        _ => {}
    }
    if let Some(id) = &c.tid {
        cmd.args(["-s", id]);
    } else if c.last {
        cmd.arg("-c");
    }
    cmd.args(&c.args);
    // the message goes before -f, because -f takes a list and would swallow it
    cmd.arg(x);
    for (k, (m, b)) in imgs.iter().enumerate() {
        let f = std::env::temp_dir().join(format!("noname-{}-{t}-{k}.{}", std::process::id(), crate::img::ext(m)));
        if std::fs::write(&f, b).is_ok() {
            cmd.arg("-f").arg(f);
        }
    }
    cmd
}

fn base(p: &str) -> String {
    p.rsplit('/').next().unwrap_or(p).to_string()
}

impl App {
    pub(crate) fn ocev(&mut self, e: Value) {
        let ty = sv(&e, "type");
        if let Some(id) = e["sessionID"].as_str() {
            if let Some(c) = &mut self.cx {
                if c.tid.is_none() {
                    c.tid = Some(id.to_string());
                }
            }
        }
        let p = &e["part"];
        match ty.as_str() {
            "text" | "reasoning" => {
                let txt = sv(p, "text");
                if txt.trim().is_empty() {
                    return;
                }
                let id = sv(p, "id");
                let b = if ty == "reasoning" { B::Th(txt.trim().to_string()) } else { B::T(txt.trim().to_string()) };
                match self.cx.as_ref().and_then(|c| c.im.get(&id).copied()) {
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
            "tool_use" => self.octool(p),
            "step_finish" => {
                let n = p["tokens"]["total"].as_u64().unwrap_or(0);
                if let Some(c) = &mut self.cx {
                    c.tok += n;
                }
            }
            "error" => {
                let m = ["/error/data/message", "/error/message", "/error/name"].iter().find_map(|k| e.pointer(k).and_then(|x| x.as_str())).unwrap_or("error");
                self.bs.push(B::T(format!("**opencode:** {}", one(m, 200))));
            }
            "_eof" => {
                let (tok, t0) = self.cx.as_ref().map(|c| (c.tok, c.t0)).unwrap_or((0, self.t));
                if tok > 0 {
                    let secs = (self.t - t0) as f32 * 0.033;
                    let tk = if tok >= 1000 { format!("{:.1}k", tok as f64 / 1000.0) } else { tok.to_string() };
                    self.bs.push(B::Rc(format!("{secs:.1}s  ·  {tk} tok"), self.t.max(1)));
                }
                self.cev(e);
            }
            _ => {}
        }
    }

    fn octool(&mut self, p: &Value) {
        let nm = sv(p, "tool");
        let s = &p["state"];
        let inp = &s["input"];
        if nm == "todowrite" {
            if let Some(a) = inp["todos"].as_array() {
                self.todo = a.iter().map(|x| (match sv(x, "status").as_str() { "completed" => 2, "in_progress" => 1, _ => 0 }, sv(x, "content"))).collect();
            }
            return;
        }
        let path = {
            let f = sv(inp, "filePath");
            if f.is_empty() { sv(inp, "path") } else { f }
        };
        let (k, a) = match nm.as_str() {
            "bash" => (Tk::Bash, one(&sv(inp, "command"), 140)),
            "edit" | "write" | "patch" | "multiedit" | "apply_patch" => (Tk::Edit, base(&path)),
            "read" | "list" => (Tk::Read, base(&path)),
            "grep" | "glob" => (Tk::Grep, sv(inp, "pattern")),
            "webfetch" | "websearch" | "codesearch" => (Tk::Web, {
                let u = sv(inp, "url");
                if u.is_empty() { sv(inp, "query") } else { u }
            }),
            "task" => (Tk::Task, one(&sv(inp, "description"), 120)),
            _ => (Tk::Oth, one(&inp.to_string(), 80)),
        };
        let st = match sv(s, "status").as_str() {
            "error" => St::Err,
            "completed" => St::Ok,
            _ => St::Run,
        };
        let out = if st == St::Err { sv(s, "error") } else { sv(s, "output") };
        let ls: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
        let exit = s["metadata"]["exit"].as_i64().unwrap_or(0);
        let sum = match (k, st) {
            (_, St::Run) => String::new(),
            (Tk::Bash, _) if exit != 0 => format!("exit {exit}"),
            (_, St::Err) if out.contains("rejected permission") => "not allowed here, use auto or bypass mode".into(),
            (_, St::Err) => ls.first().map(|l| one(l, 48)).unwrap_or_else(|| "failed".into()),
            (Tk::Bash, _) => ls.last().map(|l| one(l, 48)).unwrap_or_else(|| "ok".into()),
            _ => "done".into(),
        };
        let st = if k == Tk::Bash && exit != 0 { St::Err } else { st };
        let body: Vec<Ln> = if k == Tk::Bash { ls.iter().rev().take(12).rev().map(|l| Ln::P(one(l, 200))).collect() } else { vec![] };
        let id = {
            let c = sv(p, "callID");
            if c.is_empty() { sv(p, "id") } else { c }
        };
        let now = self.t.max(1);
        match self.cx.as_ref().and_then(|c| c.im.get(&id).copied()) {
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
}
