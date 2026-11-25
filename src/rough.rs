use rand::rngs::{OsRng, StdRng};
use rand::{Rng, SeedableRng};
use regex::Regex;
use std::f64::consts::TAU;
use std::sync::OnceLock;
use xmltree::{Element, XMLNode};

const SKIP: &[&str] = &["defs", "clipPath", "mask", "pattern", "filter", "marker", "symbol", "linearGradient", "radialGradient", "style", "title", "desc"];
type P = (f64, f64);

fn num_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?").unwrap())
}

fn tok_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?").unwrap())
}

pub fn fnum(v: Option<&str>, default: f64) -> f64 {
    v.and_then(|s| num_re().find(s)).and_then(|m| m.as_str().parse().ok()).unwrap_or(default)
}

fn fmt(x: f64) -> String {
    let s = format!("{x:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".into() } else { s.to_string() }
}

fn pt(p: P) -> String {
    format!("{},{}", fmt(p.0), fmt(p.1))
}

fn lerp(a: P, b: P, t: f64) -> P {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

type Cubic = (P, P, P, P);

fn split_cubic(p0: P, c1: P, c2: P, p1: P) -> (Cubic, Cubic) {
    let (a, b, c) = (lerp(p0, c1, 0.5), lerp(c1, c2, 0.5), lerp(c2, p1, 0.5));
    let (d, e) = (lerp(a, b, 0.5), lerp(b, c, 0.5));
    let m = lerp(d, e, 0.5);
    ((p0, a, d, m), (m, e, c, p1))
}

enum Seg {
    M(P),
    L(P),
    C(P, P, P),
    A(f64, f64, f64, i32, i32, P),
    Z,
}

fn args_of(c: char) -> usize {
    match c {
        'm' | 'l' | 't' => 2,
        'h' | 'v' => 1,
        'c' => 6,
        's' | 'q' => 4,
        'a' => 7,
        _ => 0,
    }
}

fn parse_path(d: &str) -> Vec<Seg> {
    let mut toks: Vec<String> = tok_re().find_iter(d).map(|m| m.as_str().to_string()).collect();
    let mut segs = vec![];
    let mut cmd: Option<char> = None;
    let mut cur: P = (0.0, 0.0);
    let mut start = cur;
    let mut last_c: Option<P> = None;
    let mut last_q: Option<P> = None;
    let mut i = 0;
    let alpha = |s: &str| s.chars().next().map_or(false, |c| c.is_alphabetic());
    while i < toks.len() {
        if alpha(&toks[i]) {
            cmd = toks[i].chars().next();
            i += 1;
            if matches!(cmd, Some('Z') | Some('z')) {
                segs.push(Seg::Z);
                cur = start;
                last_c = None;
                last_q = None;
            }
            continue;
        }
        let Some(cm) = cmd else { i += 1; continue };
        if matches!(cm, 'Z' | 'z') {
            i += 1;
            continue;
        }
        let c = cm.to_ascii_lowercase();
        let n = args_of(c);
        let mut a: Vec<f64> = vec![];
        let mut j = i;
        while a.len() < n && j < toks.len() && !alpha(&toks[j]) {
            let t = toks[j].clone();
            if c == 'a' && (a.len() == 3 || a.len() == 4) && t.len() > 1 && (t.starts_with('0') || t.starts_with('1')) {
                a.push(t[..1].parse().unwrap());
                toks[j] = t[1..].to_string();
                continue;
            }
            a.push(t.parse().unwrap_or(0.0));
            j += 1;
        }
        if a.len() < n {
            break;
        }
        i = j;
        let (ox, oy) = if cm.is_lowercase() { cur } else { (0.0, 0.0) };
        let mut new_c = None;
        let mut new_q = None;
        let p: P;
        match c {
            'm' => {
                p = (a[0] + ox, a[1] + oy);
                segs.push(Seg::M(p));
                start = p;
                cmd = Some(if cm.is_lowercase() { 'l' } else { 'L' });
            }
            'l' => {
                p = (a[0] + ox, a[1] + oy);
                segs.push(Seg::L(p));
            }
            'h' => {
                p = (a[0] + ox, cur.1);
                segs.push(Seg::L(p));
            }
            'v' => {
                p = (cur.0, a[0] + oy);
                segs.push(Seg::L(p));
            }
            'c' | 's' => {
                let (c1, rest): (P, &[f64]) = if c == 'c' {
                    ((a[0] + ox, a[1] + oy), &a[2..])
                } else {
                    (last_c.map_or(cur, |l| (2.0 * cur.0 - l.0, 2.0 * cur.1 - l.1)), &a[..])
                };
                let c2 = (rest[0] + ox, rest[1] + oy);
                p = (rest[2] + ox, rest[3] + oy);
                segs.push(Seg::C(c1, c2, p));
                new_c = Some(c2);
            }
            'q' | 't' => {
                let q1: P;
                if c == 'q' {
                    q1 = (a[0] + ox, a[1] + oy);
                    p = (a[2] + ox, a[3] + oy);
                } else {
                    q1 = last_q.map_or(cur, |l| (2.0 * cur.0 - l.0, 2.0 * cur.1 - l.1));
                    p = (a[0] + ox, a[1] + oy);
                }
                segs.push(Seg::C(lerp(cur, q1, 2.0 / 3.0), lerp(p, q1, 2.0 / 3.0), p));
                new_q = Some(q1);
            }
            _ => {
                p = (a[5] + ox, a[6] + oy);
                segs.push(Seg::A(a[0], a[1], a[2], a[3] as i32, a[4] as i32, p));
            }
        }
        cur = p;
        last_c = new_c;
        last_q = new_q;
    }
    segs
}

pub struct Rougher<'a> {
    amp: f64,
    rng: &'a mut StdRng,
    bx: [f64; 4],
    free: bool,
    filter_id: Option<String>,
    filter_needed: bool,
}

fn style_get(el: &Element, key: &str) -> Option<String> {
    if let Some(st) = el.attributes.get("style") {
        for part in st.split(';') {
            if let Some((k, v)) = part.split_once(':') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    el.attributes.get(key).cloned()
}

fn attr(el: &Element, k: &str) -> Option<String> {
    el.attributes.get(k).cloned()
}

fn itertext(el: &Element) -> String {
    let mut s = String::new();
    for c in &el.children {
        match c {
            XMLNode::Text(t) | XMLNode::CData(t) => s.push_str(t),
            XMLNode::Element(e) => s.push_str(&itertext(e)),
            _ => {}
        }
    }
    s
}

impl<'a> Rougher<'a> {
    fn gauss(&mut self) -> f64 {
        let u1: f64 = 1.0 - self.rng.gen::<f64>();
        let u2: f64 = self.rng.gen::<f64>();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }

    fn j(&mut self, scale: f64) -> f64 {
        self.gauss() * self.amp * scale
    }

    fn jx(&mut self, x: f64, scale: f64) -> f64 {
        let (x0, w) = (self.bx[0], self.bx[2]);
        let d = self.j(scale);
        if self.free && x <= x0 + 1.0 {
            return x - d.abs() - 1.0;
        }
        if self.free && x >= x0 + w - 1.0 {
            return x + d.abs() + 1.0;
        }
        x + d
    }

    fn jy(&mut self, y: f64, scale: f64) -> f64 {
        let (y0, h) = (self.bx[1], self.bx[3]);
        let d = self.j(scale);
        if self.free && y <= y0 + 1.0 {
            return y - d.abs() - 1.0;
        }
        if self.free && y >= y0 + h - 1.0 {
            return y + d.abs() + 1.0;
        }
        y + d
    }

    fn jp(&mut self, p: P) -> P {
        (self.jx(p.0, 0.6), self.jy(p.1, 0.6))
    }

    fn steps(&self, a: P, b: P) -> usize {
        ((b.0 - a.0).hypot(b.1 - a.1) / (self.amp * 18.0 + 1e-6)).floor().max(1.0) as usize
    }

    fn wobble(&mut self, pts: &[P], closed: bool) -> String {
        let mut seq: Vec<P> = pts.to_vec();
        if closed {
            seq.push(pts[0]);
        }
        let mut out = vec![];
        for w in seq.windows(2) {
            let n = self.steps(w[0], w[1]);
            for k in 0..n {
                let q = lerp(w[0], w[1], k as f64 / n as f64);
                out.push(self.jp(q));
            }
        }
        if !closed {
            let l = *seq.last().unwrap();
            out.push(self.jp(l));
        }
        let d = format!("M{}", out.iter().map(|p| pt(*p)).collect::<Vec<_>>().join(" L"));
        if closed { d + "Z" } else { d }
    }

    fn to_path(&self, el: &mut Element, d: String) {
        const DROP: &[&str] = &["x", "y", "width", "height", "x1", "y1", "x2", "y2", "points", "cx", "cy", "r", "rx", "ry"];
        let keep: Vec<(String, String)> = el.attributes.iter().filter(|(k, _)| !DROP.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        el.children.clear();
        el.attributes.clear();
        el.name = "path".into();
        for (k, v) in keep {
            el.attributes.insert(k, v);
        }
        el.attributes.insert("d".into(), d);
    }

    fn shift(&mut self, el: &mut Element) {
        let (dx, dy) = (self.j(0.8), self.j(0.8));
        let tr = attr(el, "transform").unwrap_or_default();
        el.attributes.insert("transform".into(), format!("translate({},{}) {}", fmt(dx), fmt(dy), tr).trim().to_string());
    }

    fn line(&mut self, el: &mut Element) {
        let g = |k: &str| fnum(el.attributes.get(k).map(|s| s.as_str()), 0.0);
        let p = [(g("x1"), g("y1")), (g("x2"), g("y2"))];
        let d = self.wobble(&p, false);
        self.to_path(el, d);
        self.shift(el);
    }

    fn rect(&mut self, el: &mut Element) {
        let g = |k: &str| fnum(el.attributes.get(k).map(|s| s.as_str()), 0.0);
        if g("rx") != 0.0 || g("ry") != 0.0 {
            return;
        }
        let (x, y, w, h) = (g("x"), g("y"), g("width"), g("height"));
        if w <= 0.0 || h <= 0.0 || el.attributes.get("width").map_or(false, |s| s.ends_with('%')) {
            return;
        }
        if w >= 0.9 * self.bx[2] && h >= 0.9 * self.bx[3] {
            return;
        }
        let d = self.wobble(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], true);
        self.to_path(el, d);
    }

    fn poly(&mut self, el: &mut Element, closed: bool) {
        let nums: Vec<f64> = num_re().find_iter(el.attributes.get("points").map_or("", |s| s.as_str())).filter_map(|m| m.as_str().parse().ok()).collect();
        let pts: Vec<P> = nums.chunks_exact(2).map(|c| (c[0], c[1])).collect();
        if pts.len() >= 2 {
            let d = self.wobble(&pts, closed);
            self.to_path(el, d);
        }
    }

    fn ellipse(&mut self, el: &mut Element) {
        let g = |k: &str| fnum(el.attributes.get(k).map(|s| s.as_str()), 0.0);
        let (cx, cy) = (g("cx"), g("cy"));
        let (rx, ry) = if el.name == "circle" { (g("r"), g("r")) } else { (g("rx"), g("ry")) };
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let m = rx.max(ry);
        let n = ((TAU * m / (self.amp * 10.0 + 1e-6)) as usize).max(12).min(160);
        let drift = self.rng.gen_range(0.0..TAU);
        let mut pts = vec![];
        for k in 0..n {
            let a = TAU * k as f64 / n as f64;
            let r = 1.0 + 0.5 * self.j(1.0 / m) + 0.015 * (3.0 * a + drift).sin();
            pts.push((cx + rx * r * a.cos(), cy + ry * r * a.sin()));
        }
        let d = format!("M{}Z", pts.iter().map(|p| pt(*p)).collect::<Vec<_>>().join(" L"));
        let small = m < self.amp * 6.0;
        self.to_path(el, d);
        if small {
            self.shift(el);
        }
    }

    fn straight(&mut self, out: &mut Vec<String>, a: P, b: P) {
        let n = self.steps(a, b);
        for k in 1..n {
            let q = lerp(a, b, k as f64 / n as f64);
            let j = self.jp(q);
            out.push(format!("L{}", pt(j)));
        }
    }

    fn curve(&mut self, out: &mut Vec<String>, seg: Cubic, ja: P, jb: P, depth: u32) {
        let (p0, c1, c2, p1) = seg;
        if depth < 3 && (p1.0 - p0.0).hypot(p1.1 - p0.1) > self.amp * 30.0 {
            let (l, r) = split_cubic(p0, c1, c2, p1);
            let jm = self.jp(l.3);
            self.curve(out, l, ja, jm, depth + 1);
            self.curve(out, r, jm, jb, depth + 1);
            return;
        }
        let da = (ja.0 - p0.0, ja.1 - p0.1);
        let db = (jb.0 - p1.0, jb.1 - p1.1);
        let k1 = (c1.0 + da.0 + self.j(0.35), c1.1 + da.1 + self.j(0.35));
        let k2 = (c2.0 + db.0 + self.j(0.35), c2.1 + db.1 + self.j(0.35));
        out.push(format!("C{} {} {}", pt(k1), pt(k2), pt(jb)));
    }

    fn path(&mut self, el: &mut Element) {
        let segs = parse_path(el.attributes.get("d").map_or("", |s| s.as_str()));
        if !matches!(segs.first(), Some(Seg::M(_))) {
            return;
        }
        let mut out: Vec<String> = vec![];
        let mut cur: P = (0.0, 0.0);
        let mut start = cur;
        let mut jcur = cur;
        let mut jstart = cur;
        for s in segs {
            match s {
                Seg::M(p) => {
                    cur = p;
                    start = p;
                    jcur = self.jp(cur);
                    jstart = jcur;
                    out.push(format!("M{}", pt(jcur)));
                }
                Seg::L(p) => {
                    self.straight(&mut out, cur, p);
                    cur = p;
                    jcur = self.jp(p);
                    out.push(format!("L{}", pt(jcur)));
                }
                Seg::C(c1, c2, p) => {
                    let jend = self.jp(p);
                    self.curve(&mut out, (cur, c1, c2, p), jcur, jend, 0);
                    cur = p;
                    jcur = jend;
                }
                Seg::A(rx, ry, rot, large, sweep, p) => {
                    jcur = self.jp(p);
                    out.push(format!("A{} {} {} {} {} {}", fmt(rx), fmt(ry), fmt(rot), large, sweep, pt(jcur)));
                    cur = p;
                }
                Seg::Z => {
                    self.straight(&mut out, cur, start);
                    out.push("Z".into());
                    cur = start;
                    jcur = jstart;
                }
            }
        }
        el.attributes.insert("d".into(), out.join(" "));
    }

    fn filter_id(&mut self, used: &std::collections::HashSet<String>) -> String {
        if let Some(f) = &self.filter_id {
            return f.clone();
        }
        let (mut fid, mut k) = ("rough-edge".to_string(), 2);
        while used.contains(&fid) {
            fid = format!("rough-edge-{k}");
            k += 1;
        }
        self.filter_id = Some(fid.clone());
        fid
    }

    fn shake(&mut self, el: &mut Element, n: usize, k: f64) {
        if n == 0 {
            return;
        }
        if !el.attributes.contains_key("rotate") {
            let v: Vec<String> = (0..n).map(|_| fmt(self.gauss() * 2.2 * k)).collect();
            el.attributes.insert("rotate".into(), v.join(" "));
        }
        let own = attr(el, "dy").unwrap_or_default();
        if num_re().find_iter(&own).count() > 1 {
            return;
        }
        let offs: Vec<f64> = (0..n).map(|_| self.gauss() * self.amp * 0.5 * k).collect();
        let base = fnum(Some(&own), 0.0);
        let mut dys = vec![offs[0] + base];
        for i in 1..n {
            dys.push(offs[i] - offs[i - 1]);
        }
        el.attributes.insert("dy".into(), dys.iter().map(|d| fmt(*d)).collect::<Vec<_>>().join(" "));
    }

    fn text(&mut self, el: &mut Element, size: f64, used: &std::collections::HashSet<String>) {
        let scale = |s: f64| if s < 16.0 { 0.25 } else if s < 32.0 { 0.6 } else { 1.0 };
        let has_kids = el.children.iter().any(|c| matches!(c, XMLNode::Element(_)));
        if !has_kids {
            let n = itertext(el).chars().count();
            self.shake(el, n, scale(size));
        } else {
            self.shake_tspans(el, size, scale);
        }
        if size >= 28.0 && !el.attributes.contains_key("filter") && !itertext(el).trim().is_empty() {
            let id = self.filter_id(used);
            self.filter_needed = true;
            el.attributes.insert("filter".into(), format!("url(#{id})"));
        }
    }

    fn shake_tspans(&mut self, el: &mut Element, size: f64, scale: fn(f64) -> f64) {
        for c in el.children.iter_mut() {
            if let XMLNode::Element(t) = c {
                let leaf = !t.children.iter().any(|x| matches!(x, XMLNode::Element(_)));
                if t.name == "tspan" && leaf {
                    let ts = style_get(t, "font-size");
                    let tsize = ts.map_or(size, |s| fnum(Some(&s), size));
                    let n = itertext(t).chars().count();
                    self.shake(t, n, scale(tsize));
                } else {
                    self.shake_tspans(t, size, scale);
                }
            }
        }
    }

    fn walk(&mut self, el: &mut Element, size: f64, used: &std::collections::HashSet<String>) {
        let mut size = size;
        if let Some(fs) = style_get(el, "font-size") {
            size = fnum(Some(&fs), size);
        }
        for node in el.children.iter_mut() {
            let XMLNode::Element(child) = node else { continue };
            if SKIP.contains(&child.name.as_str()) || child.attributes.get("data-rough").map(|s| s.as_str()) == Some("off") {
                continue;
            }
            let was = self.free;
            if child.attributes.contains_key("transform") {
                self.free = false;
            }
            if child.name == "text" {
                let cs = style_get(child, "font-size");
                let sz = cs.map_or(size, |s| fnum(Some(&s), size));
                self.text(child, sz, used);
            } else {
                match child.name.as_str() {
                    "line" => self.line(child),
                    "rect" => self.rect(child),
                    "polyline" => self.poly(child, false),
                    "polygon" => self.poly(child, true),
                    "circle" | "ellipse" => self.ellipse(child),
                    "path" => self.path(child),
                    _ => {}
                }
                self.walk(child, size, used);
            }
            self.free = was;
        }
    }
}

fn ids(el: &Element, set: &mut std::collections::HashSet<String>) {
    if let Some(i) = el.attributes.get("id") {
        set.insert(i.clone());
    }
    for c in &el.children {
        if let XMLNode::Element(e) = c {
            ids(e, set);
        }
    }
}

pub fn run(src: &str, dst: &str, amount: f64, seed: Option<u64>) -> Result<(), String> {
    let text = std::fs::read_to_string(src).map_err(|e| format!("{src}: {e}"))?;
    let mut root = Element::parse(text.as_bytes()).map_err(|e| format!("{src}: {e}"))?;
    let vb: Vec<f64> = num_re().find_iter(root.attributes.get("viewBox").map_or("", |s| s.as_str())).filter_map(|m| m.as_str().parse().ok()).collect();
    let bx = if vb.len() == 4 {
        [vb[0], vb[1], vb[2], vb[3]]
    } else {
        [0.0, 0.0, fnum(root.attributes.get("width").map(|s| s.as_str()), 600.0), fnum(root.attributes.get("height").map(|s| s.as_str()), 600.0)]
    };
    let amp = amount * bx[2].hypot(bx[3]) / 720.0 * 1.4;
    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_rng(OsRng).map_err(|e| e.to_string())?,
    };
    let mut used = std::collections::HashSet::new();
    ids(&root, &mut used);
    let mut r = Rougher { amp, rng: &mut rng, bx, free: true, filter_id: None, filter_needed: false };
    r.walk(&mut root, 16.0, &used);
    if let (true, Some(fid)) = (r.filter_needed, r.filter_id.clone()) {
        let turb_seed = r.rng.gen_range(1..=9999);
        let scale = fmt(amp * 2.2);
        let mut f = Element::new("filter");
        f.namespace = root.namespace.clone();
        for (k, v) in [("id", fid.as_str()), ("x", "-5%"), ("y", "-5%"), ("width", "110%"), ("height", "110%")] {
            f.attributes.insert(k.into(), v.into());
        }
        let mk = |name: &str, kv: &[(&str, String)]| {
            let mut e = Element::new(name);
            e.namespace = root.namespace.clone();
            for (k, v) in kv {
                e.attributes.insert((*k).into(), v.clone());
            }
            XMLNode::Element(e)
        };
        f.children.push(mk("feTurbulence", &[("type", "fractalNoise".into()), ("baseFrequency", "0.9".into()), ("numOctaves", "2".into()), ("seed", turb_seed.to_string()), ("result", "n".into())]));
        f.children.push(mk("feDisplacementMap", &[("in", "SourceGraphic".into()), ("in2", "n".into()), ("scale", scale), ("xChannelSelector", "R".into()), ("yChannelSelector", "G".into())]));
        let pos = root.children.iter().position(|c| matches!(c, XMLNode::Element(e) if e.name == "defs"));
        match pos {
            Some(i) => {
                if let XMLNode::Element(d) = &mut root.children[i] {
                    d.children.push(XMLNode::Element(f));
                }
            }
            None => {
                let mut d = Element::new("defs");
                d.namespace = root.namespace.clone();
                d.children.push(XMLNode::Element(f));
                root.children.insert(0, XMLNode::Element(d));
            }
        }
    }
    let mut buf = vec![];
    let cfg = xmltree::EmitterConfig::new().write_document_declaration(false).perform_indent(false);
    root.write_with_config(&mut buf, cfg).map_err(|e| e.to_string())?;
    std::fs::write(dst, buf).map_err(|e| format!("{dst}: {e}"))?;
    println!("wrote {dst} (amp {amp:.2})");
    Ok(())
}
