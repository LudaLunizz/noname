use noname::pal::*;
use noname::*;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

struct V1 {
    db: [f32; 3],
    lt: u64,
    sf: f32,
    bg: Option<noname::bg::Bg>,
}

impl Clone for V1 {
    fn clone(&self) -> Self {
        V1 { db: self.db, lt: self.lt, sf: self.sf, bg: None }
    }
}

fn gd() -> [Color; 3] {
    [LI(), L2(), CY()]
}

fn tail_nz(v: &mut Vec<Line<'static>>, a: &App, seed: u64) {
    if let Some(l) = v.last_mut() {
        l.spans.push(pad(1));
        l.spans.extend(noise(3, a.t, seed, &gd()));
    }
}

fn sy(k: Tk) -> (&'static str, Color) {
    match k {
        Tk::Read => (".", CY()),
        Tk::Edit => ("+", GR()),
        Tk::Bash => ("$", LI()),
        Tk::Grep => ("?", CY()),
        Tk::Web => ("@", L2()),
        Tk::Task => ("&", PK()),
        Tk::Oth => ("*", PE()),
    }
}

fn mdt() -> Md {
    Md {
        t: st(FG()),
        b: st(WH()).bold(),
        c: st(L2()),
        fe: st(PE()),
        kw: st(LI()),
        st: st(GR()),
        cm: st(FA()).italic(),
        nu: st(PK()),
        bul: "- ",
        bs: st(LI()),
        fpre: "  | ",
        fps: st(FA()),
    }
}

fn glitch(v: Vec<Span<'static>>, el: u64, t: u64) -> Vec<Span<'static>> {
    if el >= 16 {
        return v;
    }
    let mut o = vec![];
    if (t / 2) % 3 == 1 {
        o.push(pad(1));
    }
    let mut k = 0u64;
    for sp in v {
        let mut run = String::new();
        for c in sp.content.chars() {
            k += 1;
            if c != ' ' && hsh(t / 2, k) % 6 == 0 {
                if !run.is_empty() {
                    o.push(Span::styled(std::mem::take(&mut run), sp.style));
                }
                let gc = if hsh(t, k) % 2 == 0 { RD() } else { PK() };
                o.push(Span::styled(nz(t, k).to_string(), st(gc).bold()));
            } else {
                run.push(c);
            }
        }
        if !run.is_empty() {
            o.push(Span::styled(run, sp.style));
        }
    }
    o
}

fn menu(a: &App) -> Vec<(String, String)> {
    let Some(q) = a.inp.strip_prefix('/') else { return vec![] };
    if q.contains(' ') || a.inp.contains('\n') {
        return vec![];
    }
    let mut cs = a.cmds();
    cs.dedup_by(|x, y| x.0 == y.0);
    let mut p: Vec<(String, String)> = cs.iter().filter(|(c, _)| c.starts_with(q)).cloned().collect();
    p.sort_by_key(|(c, _)| (c.contains(':'), c.len()));
    let mut r: Vec<(String, String)> = cs.iter().filter(|(c, _)| !c.starts_with(q) && c.contains(q)).cloned().collect();
    r.sort_by_key(|(c, _)| (c.contains(':'), c.len()));
    p.extend(r);
    if p.len() == 1 && p[0].0 == q {
        return vec![];
    }
    p
}

fn ghost(a: &App) -> Option<(String, String)> {
    let q = a.inp.strip_prefix('/')?;
    if a.cur != a.inp.chars().count() {
        return None;
    }
    let m = menu(a);
    let (c, d) = m.get(a.ms.min(m.len().saturating_sub(1)))?;
    let r = c.strip_prefix(q)?;
    Some((r.to_string(), d.clone()))
}

fn unlig(v: Vec<Span<'static>>) -> Vec<Span<'static>> {
    let mut o = vec![];
    for sp in v {
        for c in sp.content.chars() {
            let s = if c == '\\' { sp.style.remove_modifier(Modifier::BOLD) } else { sp.style.add_modifier(Modifier::BOLD) };
            o.push(Span::styled(c.to_string(), s));
        }
    }
    o
}

fn fnum(n: usize) -> String {
    if n == 0 { String::new() } else { n.to_string() }
}

fn pad(n: usize) -> Span<'static> {
    Span::raw(" ".repeat(n))
}

fn hid(a: &App, i: usize, b: &B) -> bool {
    match b {
        B::Th(s) => s.trim().is_empty() && a.streaming() != Some(i),
        B::Ask(k) => matches!(k.ans, Some('y') | Some('a')),
        B::Tl(t) => t.nm == "askuserquestion",
        _ => false,
    }
}

fn gok(b: &B) -> bool {
    matches!(b, B::Tl(t) if matches!(t.k, Tk::Read | Tk::Bash | Tk::Grep | Tk::Web | Tk::Oth) && t.st != St::Err && t.nm != "askuserquestion")
}

fn groups(a: &App) -> Vec<Option<(usize, Vec<usize>)>> {
    let n = a.bs.len();
    let mut g: Vec<Option<(usize, Vec<usize>)>> = (0..n).map(|_| None).collect();
    if a.open {
        return g;
    }
    let mut i = 0;
    while i < n {
        if !gok(&a.bs[i]) {
            i += 1;
            continue;
        }
        let mut m = vec![i];
        let mut j = i + 1;
        while j < n {
            if gok(&a.bs[j]) {
                m.push(j);
            } else if !hid(a, j, &a.bs[j]) {
                break;
            }
            j += 1;
        }
        if m.len() >= 2 {
            for &x in &m {
                g[x] = Some((i, vec![]));
            }
            g[i] = Some((i, m));
        }
        i = j;
    }
    g
}

fn gline(a: &App, ix: &[usize], w: usize) -> Vec<Line<'static>> {
    let ts: Vec<&noname::Tool> = ix.iter().filter_map(|&i| if let B::Tl(t) = &a.bs[i] { Some(t) } else { None }).collect();
    let run = ts.iter().any(|t| t.st == St::Run);
    let cnt = |k: Tk| ts.iter().filter(|t| t.k == k).count();
    let mut rd: Vec<&str> = ts.iter().filter(|t| t.k == Tk::Read).map(|t| t.a.as_str()).collect();
    rd.sort();
    rd.dedup();
    let parts: [(usize, [&str; 2], &str, &str); 5] = [
        (cnt(Tk::Bash), ["ran", "running"], "command", "commands"),
        (rd.len(), ["read", "reading"], "file", "files"),
        (cnt(Tk::Grep), ["searched for", "searching for"], "pattern", "patterns"),
        (cnt(Tk::Web), ["fetched", "fetching"], "page", "pages"),
        (cnt(Tk::Oth), ["called", "calling"], "tool", "tools"),
    ];
    let sd = ix[0] as u64 * 7 + 3;
    let mut l = vec![pad(2)];
    if run {
        l.extend(noise(1, a.t, sd, &gd()));
    } else {
        let (c, col) = sy(ts.last().map(|t| t.k).unwrap_or(Tk::Oth));
        l.push(Span::styled(c, st(col).bold()));
    }
    l.push(pad(1));
    let mut first = true;
    for (n, vb, one, many) in parts {
        if n == 0 {
            continue;
        }
        if !first {
            l.push(Span::styled(", ", st(MU())));
        }
        let v = vb[run as usize];
        let v = if first { format!("{}{}", v[..1].to_uppercase(), &v[1..]) } else { v.to_string() };
        first = false;
        l.push(Span::styled(format!("{v} "), st(MU())));
        l.push(Span::styled(n.to_string(), st(FG()).bold()));
        l.push(Span::styled(format!(" {}", if n == 1 { one } else { many }), st(MU())));
    }
    let t0 = ts.iter().map(|t| t.t0).filter(|&x| x > 0).min().unwrap_or(0);
    let r = if run {
        let mut r = noise(6, a.t, sd + 1, &gd());
        r.push(Span::styled(format!("  {:.1}s", a.t.saturating_sub(t0) as f32 / 30.0), st(FA())));
        r
    } else {
        vec![]
    };
    let mut v = vec![lr(l, r, w)];
    if run {
        if let Some(t) = ts.iter().rev().find(|t| t.st == St::Run) {
            let (c, _) = sy(t.k);
            let mut s = t.a.clone();
            let mx = w.saturating_sub(10);
            if s.chars().count() > mx {
                s = s.chars().take(mx.saturating_sub(1)).collect::<String>() + "~";
            }
            v.push(Line::from(vec![pad(4), Span::styled(format!("{c} "), st(FA())), Span::styled(s, st(FA()))]));
        }
    }
    v
}

fn body(a: &App, w: usize) -> Vec<Line<'static>> {
    let m = mdt();
    let mut v: Vec<Line<'static>> = vec![];
    let mut pk = 0u8;
    let lu = a.bs.iter().rposition(|b| matches!(b, B::U(..)));
    let fly = a.ts > 0 && a.hist.last().is_some_and(|h| !h.starts_with('/'));
    let gs = groups(a);
    for (i, b) in a.bs.iter().enumerate() {
        if hid(a, i, b) {
            continue;
        }
        if let Some((s0, ix)) = &gs[i] {
            if *s0 != i {
                continue;
            }
            if !v.is_empty() && pk != 4 {
                v.push(Line::default());
            }
            pk = 4;
            v.extend(gline(a, ix, w));
            continue;
        }
        let k = match b {
            B::U(..) => 1,
            B::T(_) => 2,
            B::Th(_) => 3,
            B::Tl(_) => 4,
            B::Ask(_) => 7,
            B::Sys(_) | B::Qa(..) => 5,
            B::Rc(..) => 6,
        };
        if !v.is_empty() && k != 6 && (k != pk || k == 1 || k == 2) {
            v.push(Line::default());
            if k == 1 {
                v.push(Line::default());
            }
        }
        pk = k;
        match b {
            B::U(s, _) => {
                let el = if fly && lu == Some(i) { a.t.saturating_sub(a.ts) * 3 } else { u64::MAX / 2 };
                for (j, l) in wrap(s, w - 4).into_iter().enumerate() {
                    let mut sp = vec![Span::styled(if j == 0 { "> " } else { "  " }, st(LI()).bold())];
                    sp.extend(decode(&l, el.saturating_sub(j as u64 * 30), a.t, 300 + j as u64, st(WH()).bold(), &gd()));
                    v.push(Line::from(sp));
                }
            }
            B::Th(s) if s.trim().is_empty() => {
                let mut sp = vec![pad(2), Span::styled("~ ", st(FA()))];
                sp.extend(noise(6, a.t, i as u64, &gd()));
                sp.push(Span::styled("  thinking", st(FA())));
                v.push(Line::from(sp));
            }
            B::Th(s) => {
                let ls = wrap(s, w - 6);
                let n = if a.open || a.streaming() == Some(i) { ls.len() } else { 1 };
                for (j, l) in ls.iter().take(n).enumerate() {
                    let mut sp = vec![pad(2), Span::styled(if j == 0 { "~ " } else { "  " }, st(FA())), Span::styled(l.clone(), st(MU()).italic())];
                    if j == n - 1 && n < ls.len() {
                        sp.push(Span::styled(" ...", st(FA())));
                    }
                    v.push(Line::from(sp));
                }
                if a.streaming() == Some(i) {
                    tail_nz(&mut v, a, i as u64);
                }
            }
            B::T(s) => {
                for l in md(s, w - 2, &m) {
                    let mut sp = vec![pad(2)];
                    sp.extend(l.spans);
                    v.push(Line::from(sp));
                }
                if a.streaming() == Some(i) {
                    tail_nz(&mut v, a, i as u64);
                }
            }
            B::Tl(t) => {
                let (c, col) = sy(t.k);
                let sd = i as u64 * 7 + 3;
                let ea = if t.t0 > 0 { a.t.saturating_sub(t.t0) } else { a.t };
                let ef = if t.t0 > 0 { a.t.saturating_sub(t.t1) } else { a.t };
                let mut l = vec![pad(2)];
                if t.st == St::Run {
                    l.extend(noise(1, a.t, sd, &gd()));
                } else {
                    let cc = if t.st == St::Err { RD() } else { col };
                    l.push(Span::styled(if t.st == St::Err { "x" } else { c }.to_string(), st(cc).bold()));
                }
                l.push(pad(1));
                l.push(Span::styled(format!("{:<6} ", t.nm), st(MU())));
                l.extend(decode(&t.a, ea, a.t, sd, st(FG()), &gd()));
                let r = if t.st == St::Run {
                    let mut r = noise(6, a.t, sd + 1, &gd());
                    r.push(Span::styled(format!("  {:.1}s", a.secs(t)), st(FA())));
                    r
                } else if t.st == St::Err {
                    vec![Span::styled(t.sum.clone(), st(RD()))]
                } else if t.k == Tk::Edit {
                    let (ad, de) = t.sum.split_once(' ').unwrap_or((&t.sum, ""));
                    let mut r = decode(ad, ef, a.t, sd, st(GR()), &gd());
                    r.push(pad(1));
                    r.extend(decode(de, ef.saturating_sub(2), a.t, sd + 5, st(RD()), &gd()));
                    r
                } else {
                    decode(&t.sum, ef, a.t, sd, st(FA()), &gd())
                };
                let line = lr(l, r, w);
                if t.st == St::Err {
                    let el = a.t.saturating_sub(t.t1);
                    let mut sp: Vec<Span<'static>> = line.spans.into_iter().map(|x| if x.style.fg == Some(FG()) || x.style.fg == Some(MU()) { Span::styled(x.content, st(RD())) } else { x }).collect();
                    sp = glitch(sp, el, a.t);
                    v.push(Line::from(sp));
                } else {
                    v.push(line);
                }
                let show = t.st != St::Run && (t.k == Tk::Edit || t.st == St::Err || a.open);
                let auto = t.k == Tk::Edit || t.st == St::Err;
                let base = if auto { if t.t0 > 0 { t.t1 } else { 0 } } else { a.to };
                let kk = if auto && a.open { usize::MAX } else { (a.t.saturating_sub(base) as usize + 1) * 1 };
                if show {
                    let el = a.t.saturating_sub(base);
                    let nd = t.body.iter().filter(|x| matches!(x, Ln::D(..))).count() as u64;
                    let fx = t.k == Tk::Edit && !a.open;
                    let (mut jd, mut ja) = (0u64, 0u64);
                    for ln in t.body.iter().take(if fx { usize::MAX } else { kk }) {
                        let (n, s, c, mk) = match ln {
                            Ln::P(s) => (String::new(), s.clone(), if s.contains("FAIL") || s.contains("failed") { RD() } else { MU() }, ' '),
                            Ln::A(n, s) => (fnum(*n), s.clone(), GR(), '+'),
                            Ln::D(n, s) => (fnum(*n), s.clone(), RD(), '-'),
                            Ln::C(n, s) => (fnum(*n), s.clone(), FA(), ' '),
                        };
                        let mut s = s;
                        if s.chars().count() > w.saturating_sub(14) {
                            s = s.chars().take(w.saturating_sub(15)).collect::<String>() + "~";
                        }
                        let mut sp = vec![pad(4), Span::styled("| ", st(FA())), Span::styled(format!("{n:>3} "), st(FA()))];
                        match ln {
                            Ln::D(..) if fx => {
                                let ts = nd + 5 + jd;
                                jd += 1;
                                if el < jd {
                                    continue;
                                }
                                let (col, md) = if el < ts {
                                    (RD(), Modifier::empty())
                                } else {
                                    let f = ((el - ts) as f32 / 10.0).min(1.0);
                                    (grad(&[RD(), grad(&[RD(), BASE()], 0.5)], f), Modifier::CROSSED_OUT)
                                };
                                sp.push(Span::styled("- ", st(col)));
                                sp.push(Span::styled(s, st(col).add_modifier(md)));
                            }
                            Ln::A(..) if fx => {
                                let sa = if nd > 0 { nd + 7 } else { 0 } + ja * 2;
                                ja += 1;
                                if el < sa {
                                    continue;
                                }
                                let nc = ((el - sa) * 6) as usize;
                                let cs: Vec<char> = s.chars().collect();
                                sp.push(Span::styled("+ ", st(GR())));
                                if nc >= cs.len() {
                                    sp.push(Span::styled(s, st(GR())));
                                } else {
                                    sp.push(Span::styled(cs[..nc].iter().collect::<String>(), st(GR())));
                                    sp.push(Span::styled(cs[nc].to_string(), st(WH()).bold()));
                                }
                            }
                            _ => {
                                sp.push(Span::styled(format!("{mk} "), st(c)));
                                sp.push(Span::styled(s, st(c)));
                            }
                        }
                        v.push(Line::from(sp));
                    }
                    pk = 0;
                }
            }
            B::Ask(k) => match k.ans {
                None => {
                    let iw = w.min(64) - 4;
                    let pc = grad(&[PK(), grad(&[PK(), BASE()], 0.75)], pulse(a.t, 60.0));
                    v.push(Line::from(vec![pad(2), Span::styled("+- ", st(pc)), Span::styled("permission", st(PK()).bold()), Span::styled(format!(" {}+", "-".repeat(iw.saturating_sub(13))), st(pc))]));
                    let row = |sp: Vec<Span<'static>>| {
                        let n = sw(&sp);
                        let mut x = vec![pad(2), Span::styled("| ", st(pc))];
                        x.extend(sp);
                        x.push(pad(iw.saturating_sub(n + 1)));
                        x.push(Span::styled("|", st(pc)));
                        Line::from(x)
                    };
                    v.push(row(vec![]));
                    v.push(row(vec![Span::styled(k.cmd.clone(), st(WH()).bold())]));
                    v.push(row(vec![]));
                    v.push(row(vec![Span::styled("[y]", st(GR()).bold()), Span::styled(" allow   ", st(FG())), Span::styled("[n]", st(RD()).bold()), Span::styled(" deny   ", st(FG())), Span::styled("[a]", st(LI()).bold()), Span::styled(" always allow", st(FG()))]));
                    v.push(Line::from(vec![pad(2), Span::styled(format!("+{}+", "-".repeat(iw)), st(pc))]));
                }
                Some(c) => {
                    let (t, col) = if c == 'n' { ("denied", RD()) } else { ("allowed", GR()) };
                    v.push(lr(vec![pad(2), Span::styled("! ", st(PK()).bold()), Span::styled(format!("{:<6}", "ask"), st(MU())), Span::styled(k.cmd.clone(), st(FG()))], vec![Span::styled(t, st(col))], w));
                }
            },
            B::Qa(q, r) => {
                v.push(Line::from(vec![pad(2), Span::styled("? ", st(CY()).bold()), Span::styled(q.clone(), st(MU()))]));
                v.push(Line::from(vec![pad(4), Span::styled(r.clone(), st(WH()))]));
            }
            B::Rc(x, t0) => {
                let el = a.t.saturating_sub(*t0) * 2;
                let mut sp = vec![pad(2)];
                sp.extend(decode(x, el, a.t, i as u64 + 900, st(FA()), &gd()));
                v.push(Line::from(sp));
            }
            B::Sys(s) => v.push(Line::from(vec![pad(2), Span::styled("- ", st(FA())), Span::styled(s.clone(), st(MU()))])),
        }
    }
    if a.busy && a.ask.is_none() && a.streaming().is_none() && !matches!(a.bs.last(), Some(B::Tl(t)) if t.st == St::Run) {
        v.push(Line::default());
        let mut sp = vec![pad(2)];
        sp.extend(noise(8, a.t, 99, &gd()));
        sp.push(Span::styled(format!("  thinking  {:.1}s", a.turn_secs()), st(FA())));
        v.push(Line::from(sp));
    }
    v
}

impl V for V1 {
    fn key(&mut self, k: &Ke, a: &mut App) -> bool {
        let m = menu(a);
        if !m.is_empty() && a.rw.is_none() && a.rs.is_none() {
            let sel = a.ms.min(m.len() - 1);
            match k.code {
                K::Up => {
                    a.ms = sel.saturating_sub(1);
                    return true;
                }
                K::Down => {
                    a.ms = (sel + 1).min(m.len() - 1);
                    return true;
                }
                K::Tab | K::Right => {
                    a.inp = format!("/{} ", m[sel].0);
                    a.cur = a.inp.chars().count();
                    a.ms = 0;
                    return true;
                }
                K::Enter if k.modifiers.is_empty() => {
                    a.inp = format!("/{}", m[sel].0);
                    a.cur = a.inp.chars().count();
                    a.ms = 0;
                    a.submit();
                    return true;
                }
                _ => {}
            }
        }
        let end = a.cur == a.inp.chars().count();
        if matches!(k.code, K::Tab | K::Right) && end && a.rw.is_none() {
            if let Some((r, _)) = ghost(a) {
                a.inp.push_str(&r);
                a.inp.push(' ');
                a.cur = a.inp.chars().count();
                return true;
            }
            if a.inp.is_empty() {
                if let Some(x) = a.sug.take() {
                    a.inp = x;
                    a.cur = a.inp.chars().count();
                    return true;
                }
            }
            return k.code == K::Tab;
        }
        false
    }

    fn scen(&mut self, s: &str, a: &mut App) {
        match s {
            "plan" => a.mode = 1,
            "byp" => a.mode = 3,
            "morph" => {
                a.mode = 2;
                a.t = 5;
                a.tm = 1;
            }
            _ => {}
        }
        if s == "slash" {
            a.inp = "/co".into();
            a.cur = 3;
        }
    }

    fn draw(&mut self, f: &mut Frame, a: &App) {
        let ar = f.area();
        let w = (ar.width as usize).saturating_sub(6);
        let x0 = ar.x + (ar.width - w as u16) / 2;
        let iw = w - 2;
        let (il, (cx, cy)) = inwrap(&a.inp, a.cur, iw);
        let ih = il.len().clamp(1, 8);
        let mut extra: Vec<Line<'static>> = vec![];
        let mut bt: Vec<Line<'static>> = vec![];
        if let Some(b) = &a.btw {
            let l = vec![pad(2), Span::styled("btw  ", st(L2()).bold()), Span::styled(b.q.clone(), st(MU()))];
            bt.push(Line::from(l));
            for l in md(&b.a, w - 9, &mdt()) {
                let mut sp = vec![pad(7)];
                sp.extend(l.spans);
                bt.push(Line::from(sp));
            }
            if !b.done() {
                tail_nz(&mut bt, a, 77);
            }
        }
        let bwh = bt.len() as u16;
        if let Some((sel, opt)) = a.rw {
            let us = a.users();
            let n = us.len();
            let lo = sel.saturating_sub(6).min(n.saturating_sub(7));
            extra.push(Line::from(Span::styled("rewind", st(L2()).bold())));
            for (j, &bi) in us.iter().enumerate().skip(lo).take(7) {
                let t = match a.bs.get(bi) {
                    Some(B::U(t, _)) => t.lines().next().unwrap_or("").to_string(),
                    _ => String::new(),
                };
                let t: String = if t.chars().count() > w - 8 { t.chars().take(w - 9).collect::<String>() + "…" } else { t };
                let on = j == sel;
                extra.push(Line::from(vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold()), Span::styled(t, if on { st(WH()).bold() } else { st(MU()) })]));
                if on {
                    if let Some(o) = opt {
                        let mut sp = vec![pad(4)];
                        for (k, l) in ["code + conversation", "conversation", "code"].iter().enumerate() {
                            if k > 0 {
                                sp.push(pad(4));
                            }
                            if k == o {
                                sp.push(Span::styled(format!("[ {l} ]"), st(LI()).bold()));
                            } else {
                                sp.push(Span::styled(format!("  {l}  "), st(FA())));
                            }
                        }
                        extra.push(Line::from(sp));
                    }
                }
            }
            extra.push(Line::default());
        }
        let mm = menu(a);
        if !mm.is_empty() && a.rw.is_none() {
            let sel = a.ms.min(mm.len() - 1);
            let lo = sel.saturating_sub(5).min(mm.len().saturating_sub(6));
            let cw = mm.iter().skip(lo).take(6).map(|x| x.0.chars().count()).max().unwrap_or(0).min(36) + 3;
            for (j, (c, d)) in mm.iter().enumerate().skip(lo).take(6) {
                let on = j == sel;
                let c: String = c.chars().take(36).collect();
                let dw = w.saturating_sub(cw + 4);
                let d: String = if d.chars().count() > dw { d.chars().take(dw.saturating_sub(1)).collect::<String>() + "…" } else { d.clone() };
                extra.push(Line::from(vec![
                    Span::styled(if on { "› " } else { "  " }, st(LI()).bold()),
                    Span::styled(format!("/{c:<w$}", w = cw - 1), if on { st(WH()).bold() } else { st(L2()) }),
                    Span::styled(d, st(if on { MU() } else { FA() })),
                ]));
            }
            if mm.len() > 6 {
                extra.push(Line::from(Span::styled(format!("  {} more", mm.len() - 6), st(FA()))));
            }
            extra.push(Line::default());
        }
        if let Some(q) = &a.qa {
            let c = &q.qs[q.i];
            let hd = vec![Span::styled(if c.h.is_empty() { "question".to_string() } else { c.h.to_lowercase() }, st(L2()).bold())];
            let rt = if q.qs.len() > 1 { vec![Span::styled(format!("{}/{}", q.i + 1, q.qs.len()), st(FA()))] } else { vec![] };
            extra.push(lr(hd, rt, w));
            extra.push(Line::from(Span::styled(c.q.clone(), st(WH()).bold())));
            extra.push(Line::default());
            let lw = c.opts.iter().map(|o| o.0.chars().count()).max().unwrap_or(0).min(40) + 3;
            for (j, (l, d)) in c.opts.iter().enumerate() {
                let on = j == q.sel;
                let mut sp = vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold())];
                if c.multi {
                    sp.push(Span::styled(if q.pick[j] { "[x] " } else { "[ ] " }, st(if q.pick[j] { GR() } else { FA() })));
                } else {
                    sp.push(Span::styled(format!("{}  ", j + 1), st(FA())));
                }
                let l: String = l.chars().take(40).collect();
                sp.push(Span::styled(format!("{l:<lw$}"), if on { st(WH()).bold() } else { st(FG()) }));
                let dw = w.saturating_sub(lw + 10);
                let d: String = if d.chars().count() > dw { d.chars().take(dw.saturating_sub(1)).collect::<String>() + "…" } else { d.clone() };
                sp.push(Span::styled(d, st(if on { MU() } else { FA() })));
                extra.push(Line::from(sp));
            }
            let on = q.sel == c.opts.len();
            extra.push(Line::from(vec![
                Span::styled(if on { "› " } else { "  " }, st(LI()).bold()),
                Span::styled(if c.multi { "    " } else { "   " }, st(FA())),
                Span::styled("other", if on { st(WH()).bold() } else { st(MU()) }),
            ]));
            extra.push(Line::default());
        }
        if let Some(sel) = a.ns {
            extra.push(Line::from(Span::styled("noname", st(L2()).bold())));
            let rows = a.nrows();
            let cw = rows.iter().map(|r| r.0.len()).max().unwrap_or(0) + 3;
            for (j, (k, v)) in rows.iter().enumerate() {
                let on = j == sel;
                extra.push(Line::from(vec![
                    Span::styled(if on { "› " } else { "  " }, st(LI()).bold()),
                    Span::styled(format!("{k:<cw$}"), if on { st(WH()).bold() } else { st(MU()) }),
                    Span::styled(v.clone(), st(if on { L2() } else { FA() })),
                ]));
            }
            extra.push(Line::default());
        }
        if let Some(sel) = a.pp {
            extra.push(Line::from(Span::styled("default permission", st(L2()).bold())));
            for (j, m) in MODES.iter().enumerate() {
                let on = j == sel;
                extra.push(Line::from(vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold()), Span::styled(m.to_string(), if on { st(WH()).bold() } else { st(MU()) })]));
            }
            extra.push(Line::default());
        }
        if let Some((sel, _)) = a.tp {
            extra.push(Line::from(Span::styled("theme", st(L2()).bold())));
            for (j, t) in pal::THEMES.iter().enumerate() {
                let on = j == sel;
                extra.push(Line::from(vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold()), Span::styled(t.name, if on { st(WH()).bold() } else { st(MU()) })]));
            }
            extra.push(Line::default());
        }
        if let Some((sel, _)) = a.bp {
            let v = noname::bg::avail();
            extra.push(Line::from(Span::styled("background", st(L2()).bold())));
            let lo = sel.saturating_sub(8).min(v.len().saturating_sub(9));
            for (j, k) in v.iter().enumerate().skip(lo).take(9) {
                let on = j == sel;
                extra.push(Line::from(vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold()), Span::styled(k.to_string(), if on { st(WH()).bold() } else { st(MU()) })]));
            }
            extra.push(Line::default());
        }
        if let Some((all, sel)) = &a.rs {
            let v = a.rhits();
            let sel = (*sel).min(v.len().saturating_sub(1));
            let hd = vec![Span::styled("resume", st(L2()).bold()), Span::styled("  ", st(FA())), Span::styled(if a.rq.is_empty() { "type to search".to_string() } else { a.rq.clone() }, if a.rq.is_empty() { st(FA()) } else { st(WH()) })];
            extra.push(lr(hd, vec![Span::styled(format!("{}/{}", v.len(), all.len()), st(FA()))], w));
            extra.push(Line::default());
            let lo = sel.saturating_sub(5).min(v.len().saturating_sub(6));
            for (j, (_, t, meta)) in v.iter().enumerate().skip(lo).take(6) {
                let on = j == sel;
                let tw = w.saturating_sub(4);
                let t: String = if t.chars().count() > tw { t.chars().take(tw.saturating_sub(1)).collect::<String>() + "…" } else { t.clone() };
                extra.push(Line::from(vec![Span::styled(if on { "› " } else { "  " }, st(LI()).bold()), Span::styled(t, if on { st(WH()).bold() } else { st(FG()) })]));
                extra.push(Line::from(vec![pad(2), Span::styled(meta.clone(), st(if on { MU() } else { FA() }))]));
            }
            if v.is_empty() {
                extra.push(Line::from(Span::styled("  no match", st(FA()))));
            }
            extra.push(Line::default());
        }
        if a.stodo && !a.todo.is_empty() {
            for (s, t) in &a.todo {
                let (m, c) = match s {
                    2 => ("[x] ", GR()),
                    1 => ("[>] ", LI()),
                    _ => ("[ ] ", FA()),
                };
                extra.push(Line::from(vec![Span::styled(m, st(c)), Span::styled(t.clone(), st(if *s == 2 { MU() } else { FG() }))]));
            }
            extra.push(Line::default());
        }
        let eh = extra.len() as u16;
        let h = ar.height;
        let top_in = lay(0) == 1;
        let tvis = match lay(4) {
            0 => !a.bs.iter().any(|b| matches!(b, B::U(..))),
            1 => true,
            _ => false,
        };
        let tp = lay(3);
        let lgw = LOGO[0].chars().count();
        let tvis = tvis && w >= lgw + 4 && h >= 24;
        let rt = if tvis && (1..=3).contains(&tp) { 5u16 } else { 0 };
        let rb = if tvis && tp == 4 { 5u16 } else { 0 };
        let bh = h.saturating_sub(ih as u16 + bwh + eh + 6 + rt + rb);
        let (top, ey, ry, iy, bty) = if top_in {
            let iy = 2u16 + rt;
            let ry = iy + ih as u16;
            let bty = ry + 1;
            let ey = bty + bwh;
            (ey + eh + 1, ey, ry, iy, bty)
        } else {
            let top = 2u16 + rt;
            let ey = top + bh + 1;
            let ry = ey + eh;
            let iy = ry + 1;
            (top, ey, ry, iy, iy + ih as u16)
        };
        let sy = if top_in { h.saturating_sub(1) } else { bty + bwh + 1 + rb };
        let lgx = match tp {
            1 => x0,
            2 => x0 + (w - lgw.min(w)) as u16 / 2,
            _ => x0 + (w - lgw.min(w)) as u16,
        };
        let lgel = a.t.saturating_sub(a.tc) * 2;
        let lgy = if rt > 0 { Some(0u16) } else if rb > 0 { Some(sy - 5) } else { None };
        if let Some(y0) = lgy {
            for (i, l) in LOGO.iter().enumerate() {
                let sp = unlig(decode(l, lgel.saturating_sub(i as u64 * 6), a.t, i as u64 + 50, st(LI()), &gd()));
                f.render_widget(Paragraph::new(Line::from(sp)), Rect { x: lgx, y: y0 + i as u16, width: lgw as u16, height: 1 });
            }
        }
        let col = |y: u16, hh: u16| Rect { x: x0, y, width: w as u16, height: hh };


        let mid = tvis && tp == 0 && bh >= 6;
        if a.bs.is_empty() {
            if mid {
                let y = top + bh / 2 - 3;
                for (i, l) in LOGO.iter().enumerate() {
                    let sp = unlig(decode(l, lgel.saturating_sub(i as u64 * 6), a.t, i as u64 + 50, st(LI()), &gd()));
                    f.render_widget(Paragraph::new(Line::from(sp)).alignment(Alignment::Center), col(y + i as u16, 1));
                }
            }
        } else {
            let bl = body(a, w);
            let mx = bl.len().saturating_sub(bh as usize);
            a.smax.set(mx);
            let tg = a.scr.min(mx) as f32;
            let n = a.t.saturating_sub(self.lt).min(60) as i32;
            self.sf += (tg - self.sf) * (1.0 - 0.55f32.powi(n));
            if (tg - self.sf).abs() < 0.4 {
                self.sf = tg;
            }
            let mut scr = self.sf.round() as usize;
            let ls = tail(bl, bh as usize, &mut scr);
            let pad_top = bh as usize - ls.len();
            f.render_widget(Paragraph::new(ls), col(top + pad_top as u16, bh - pad_top as u16));
            if scr > 0 {
                f.render_widget(Paragraph::new(Span::styled(format!("↓ {scr}"), st(FA()))).alignment(Alignment::Right), col(top + bh - 1, 1));
            }
            if mid {
                let y0 = top + bh / 2 - 2;
                let xl = x0 + (w - lgw) as u16 / 2;
                let c = grad(&[FA(), BASE()], 0.45);
                let buf = f.buffer_mut();
                let mut put = vec![];
                let clear = (y0.saturating_sub(1)..y0 + 5).all(|y| (xl.saturating_sub(2)..xl + lgw as u16 + 2).all(|x| buf.cell((x, y)).is_none_or(|k| k.symbol() == " ")));
                for (i, l) in LOGO.iter().enumerate() {
                    for (j, ch) in l.chars().enumerate() {
                        if clear && ch != ' ' {
                            put.push((xl + j as u16, y0 + i as u16, ch));
                        }
                    }
                }
                for (x, y, ch) in put {
                    if let Some(k) = buf.cell_mut((x, y)) {
                        k.set_char(ch).set_style(st(c));
                    }
                }
            }
        }
        f.render_widget(Paragraph::new(extra), col(ey, eh));
        let rule = Span::styled("─".repeat(w), st(FA()));
        let el = a.t.saturating_sub(a.ts);
        if a.ts > 0 && el < 20 {
            let head = (el as f32 / 14.0 * (w + 16) as f32) as i64;
            let sp: Vec<Span<'static>> = (0..w as i64)
                .map(|x| {
                    let d = head - x;
                    let c = if (0..16).contains(&d) { grad(&[WH(), LI(), FA()], d as f32 / 16.0) } else { FA() };
                    Span::styled("─", st(c))
                })
                .collect();
            f.render_widget(Paragraph::new(Line::from(sp)), col(ry, 1));
        } else {
            f.render_widget(Paragraph::new(rule.clone()), col(ry, 1));
        }
        let pc = if a.ask.is_some() || a.qa.is_some() { PK() } else { [LI(), CY(), GR(), RD()][a.mode] };
        let pg: Vec<Span<'static>> = if a.tm > 0 && a.t.saturating_sub(a.tm) < 9 {
            noise(1, a.t, 61, &gd())
        } else {
            vec![Span::styled(["›", "?", "»", "!"][a.mode], st(pc).bold())]
        };
        let gh = ghost(a);
        let mut il2: Vec<Line> = vec![];
        let sk = cy.saturating_sub(ih - 1);
        for (j, l) in il.iter().enumerate().skip(sk).take(ih) {
            let mut sp = if j == 0 { pg.clone() } else { vec![pad(1)] };
            sp.push(pad(1));
            let ff = a.inp.is_empty() && a.ts > 0 && a.t.saturating_sub(a.ts) < 12 && a.hist.last().is_some_and(|h| !h.starts_with('/'));
            if j == 0 && ff {
                let el = a.t.saturating_sub(a.ts);
                let h = a.hist.last().cloned().unwrap_or_default();
                for (ci, ch) in h.chars().take(w.saturating_sub(4)).enumerate() {
                    let r = hsh(ci as u64, 7) % 10;
                    if ch == ' ' || r + 2 < el {
                        sp.push(pad(1));
                    } else if r < el + 2 {
                        sp.push(Span::styled(nz(a.t, ci as u64).to_string(), st(grad(&gd(), (ci % 12) as f32 / 12.0))));
                    } else {
                        sp.push(Span::styled(ch.to_string(), st(WH())));
                    }
                }
            } else {
                sp.extend(imgspans(l, st(WH()), st(PE()).bold()));
            }
            if j == 0 {
                if let Some((rest, _)) = &gh {
                    sp.push(Span::styled(rest.clone(), st(FA())));
                } else if a.inp.is_empty() && !a.busy && a.ask.is_none() {
                    if let Some(x) = &a.sug {
                        sp.push(Span::styled(x.lines().next().unwrap_or("").to_string(), st(FA()).italic()));
                    }
                }
            }
            let hint: Vec<Span<'static>> = if j != 0 {
                vec![]
            } else if gh.is_some() {
                vec![]
            } else {
                vec![]
            };
            il2.push(if hint.is_empty() { Line::from(sp) } else { lr(sp, hint, w) });
        }
        f.render_widget(Paragraph::new(il2), col(iy, ih as u16));
        let cyy = cy - sk;
        setcur(f, a, x0 + 2 + cx as u16, iy + cyy as u16);
        f.render_widget(Paragraph::new(bt), col(bty, bwh));
        let tg = [a.ctx, a.h5, a.d7];
        let n = a.t.saturating_sub(self.lt).min(400) as i32;
        let k = 1.0 - 0.88f32.powi(n);
        for j in 0..3 {
            self.db[j] += (tg[j].max(0.0) - self.db[j]) * k;
        }
        self.lt = a.t;
        let db = self.db;
        let (c1, c2) = bar(db[0], 10, "▰", "▱");
        let (h1, h2) = bar(db[1], 10, "▰", "▱");
        let (d1, d2) = bar(db[2], 10, "▰", "▱");
        let segs = [
            vec![Span::styled("context ", st(FA())), Span::styled(c1, st(LI())), Span::styled(c2, st(FA())), Span::styled(if tg[0] < 0.0 { " –".to_string() } else { format!(" {:.0}%", db[0]) }, st(FG()))],
            vec![Span::styled("5h ", st(FA())), Span::styled(h1, st(CY())), Span::styled(h2, st(FA())), Span::styled(if tg[1] < 0.0 { " –".to_string() } else { format!(" {:.0}%", db[1]) }, st(FG()))],
            vec![Span::styled("week ", st(FA())), Span::styled(d1, st(CY())), Span::styled(d2, st(FA())), Span::styled(if tg[2] < 0.0 { " –".to_string() } else { format!(" {:.0}%", db[2]) }, st(FG()))],
        ];
        let vis = bars();
        let shown: Vec<Vec<Span<'static>>> = segs.into_iter().enumerate().filter(|(j, _)| vis[*j]).map(|(_, x)| x).collect();
        let toast = match &a.toast {
            Some((t, e)) if !t.starts_with("mode") => decode(t, a.t + 60 - e, a.t, 13, st(L2()), &gd()),
            _ => vec![],
        };
        let mut model = if a.work { vec![Span::styled("work  ", st(PK()).bold())] } else { vec![] };
        model.extend(decode(&a.mname.replace(' ', "-"), a.t / 2, a.t, 41, st(LI()), &gd()));
        if !a.effs.is_empty() {
            model.push(Span::styled(format!("  {}", a.effs), st(FA())));
        }
        let cwd = vec![Span::styled(a.cwd.clone(), st(MU())), Span::styled(if a.br.is_empty() { String::new() } else { format!(" ({})", a.br) }, st(FA()))];
        let mut rows: [[Vec<Ch>; 3]; 2] = Default::default();
        rows[0][0].push(Ch::S(cwd));
        if !toast.is_empty() {
            rows[1][2].push(Ch::S(toast));
        }
        match lay(1) {
            0 => rows[1][0].push(Ch::U),
            1 => rows[1][2].push(Ch::U),
            2 => rows[0][0].push(Ch::U),
            3 => rows[0][2].push(Ch::U),
            _ => {}
        }
        match lay(2) {
            0 => rows[0][2].push(Ch::S(model)),
            1 => rows[1][2].push(Ch::S(model)),
            _ => {}
        }
        for (ri, y) in [(0usize, rt), (1, sy)] {
            let [l, c, r] = &rows[ri];
            let c = join(c, &shown, 2);
            let cwn = sw(&c);
            let mut mode = 2;
            let (mut ls, mut rs) = (join(l, &shown, mode), join(r, &shown, mode));
            let fits = |ls: &[Span], rs: &[Span]| if cwn > 0 { sw(ls).max(sw(rs)) * 2 + cwn + 4 <= w } else { sw(ls) + sw(rs) + 2 <= w };
            while !fits(&ls, &rs) && mode > 0 {
                mode -= 1;
                ls = join(l, &shown, mode);
                rs = join(r, &shown, mode);
            }
            let cok = cwn > 0 && fits(&ls, &rs);
            f.render_widget(Paragraph::new(lr(ls, rs, w)), col(y, 1));
            if cok {
                let cx = x0 + (w.saturating_sub(cwn) / 2) as u16;
                f.render_widget(Paragraph::new(Line::from(c)), Rect { x: cx, y, width: cwn as u16, height: 1 });
            }
        }
        if self.bg.as_ref().map(|b| b.name) != a.bgw {
            let ar = f.area();
            self.bg = a.bgw.and_then(|k| noname::bg::Bg::new(k, ar.height, ar.width).ok());
        }
        if let Some(b) = &mut self.bg {
            b.paint(f.buffer_mut(), a.t);
        }
    }
}

const LOGO: [&str; 4] = [
    r"   ____  ____  ____  ____ _____ ___  ___ ",
    r"  / __ \/ __ \/ __ \/ __ `/ __ `__ \/ _ \",
    r" / / / / /_/ / / / / /_/ / / / / / /  __/",
    r"/_/ /_/\____/_/ /_/\__,_/_/ /_/ /_/\___/ ",
];

#[derive(Clone)]
enum Ch {
    S(Vec<Span<'static>>),
    U,
}

fn join(v: &[Ch], us: &[Vec<Span<'static>>], mode: u8) -> Vec<Span<'static>> {
    let mut o: Vec<Span<'static>> = vec![];
    for x in v {
        let part: Vec<Span<'static>> = match x {
            Ch::S(s) => s.clone(),
            Ch::U if mode == 0 => vec![],
            Ch::U => {
                let mut p = vec![];
                for (j, sg) in us.iter().take(if mode == 1 { 1 } else { usize::MAX }).enumerate() {
                    if j > 0 {
                        p.push(pad(3));
                    }
                    p.extend(sg.clone());
                }
                p
            }
        };
        if part.is_empty() || sw(&part) == 0 {
            continue;
        }
        if !o.is_empty() {
            o.push(pad(4));
        }
        o.extend(part);
    }
    o
}

fn main() -> std::io::Result<()> {
    pal::load();
    run(V1 { db: [0.0; 3], lt: 0, sf: 0.0, bg: None })
}
