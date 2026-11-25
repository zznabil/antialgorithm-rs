use crate::rough::fnum;
use regex::Regex;
use resvg::{tiny_skia, usvg};
use serde_json::json;
use std::sync::Arc;
use xmltree::{Element, XMLNode};

const SHAPES: &[&str] = &["rect", "circle", "ellipse", "line", "polyline", "polygon", "path", "image", "use"];
const HOLDERS: &[&str] = &["defs", "clipPath", "mask", "pattern", "filter", "marker", "symbol"];

struct Img {
    w: usize,
    h: usize,
    rgb: Vec<f32>,
}

struct Raster {
    w: usize,
    h: usize,
    rgba: Vec<f32>,
    pixmap: tiny_skia::Pixmap,
}

fn options() -> usvg::Options<'static> {
    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();
    let mut o = usvg::Options::default();
    o.fontdb = Arc::new(db);
    o
}

fn render(svg: &str, width: u32, opt: &usvg::Options) -> Result<Raster, String> {
    let tree = usvg::Tree::from_str(svg, opt).map_err(|e| format!("svg parse: {e}"))?;
    let s = tree.size();
    let scale = width as f32 / s.width();
    let h = ((s.height() * scale).round() as u32).max(1);
    let mut pm = tiny_skia::Pixmap::new(width, h).ok_or("image too large")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let mut rgba = Vec::with_capacity((width * h * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red() as f32, c.green() as f32, c.blue() as f32, c.alpha() as f32]);
    }
    Ok(Raster { w: width as usize, h: h as usize, rgba, pixmap: pm })
}

fn flat(r: &Raster, bg: f32) -> Img {
    let mut rgb = Vec::with_capacity(r.w * r.h * 3);
    for px in r.rgba.chunks_exact(4) {
        let a = px[3] / 255.0;
        for c in 0..3 {
            rgb.push(px[c] * a + bg * (1.0 - a));
        }
    }
    Img { w: r.w, h: r.h, rgb }
}

fn lum(rgb: &[f32], i: usize) -> f32 {
    0.2126 * rgb[i * 3] + 0.7152 * rgb[i * 3 + 1] + 0.0722 * rgb[i * 3 + 2]
}

fn tag(e: &Element) -> &str {
    &e.name
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

fn label(el: &Element) -> String {
    let s = itertext(el).split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() { "(empty)".into() } else { s.chars().take(40).collect() }
}

fn hide(el: &mut Element) {
    let st = el.attributes.get("style").cloned().unwrap_or_default();
    el.attributes.insert("style".into(), if st.is_empty() { "display:none".into() } else { format!("{st};display:none") });
}

fn has_filter(el: &Element) -> bool {
    el.attributes.contains_key("filter") || el.attributes.get("style").map_or(false, |s| s.contains("filter"))
}

fn drop_filter(el: &mut Element, re: &Regex) {
    el.attributes.shift_remove("filter");
    if let Some(st) = el.attributes.get("style").cloned() {
        if st.contains("filter") {
            el.attributes.insert("style".into(), re.replace_all(&st, "").to_string());
        }
    }
}

struct TextInfo {
    own_filter: bool,
    anc_filtered: bool,
}

fn collect(el: &Element, in_holder: bool, anc: bool, out: &mut Vec<TextInfo>) {
    for c in &el.children {
        let XMLNode::Element(e) = c else { continue };
        let holder = HOLDERS.contains(&tag(e));
        if holder {
            collect(e, true, anc || has_filter(e), out);
        } else if tag(e) == "text" && !in_holder {
            out.push(TextInfo { own_filter: has_filter(e), anc_filtered: anc });
        } else {
            collect(e, in_holder, anc || has_filter(e), out);
        }
    }
}

fn each_text(el: &mut Element, in_holder: bool, f: &mut dyn FnMut(&mut Element)) {
    for c in el.children.iter_mut() {
        let XMLNode::Element(e) = c else { continue };
        if HOLDERS.contains(&tag(e)) {
            each_text(e, true, f);
        } else if tag(e) == "text" && !in_holder {
            f(e);
        } else {
            each_text(e, in_holder, f);
        }
    }
}

fn strip(el: &mut Element, re: &Regex) {
    for c in el.children.iter_mut() {
        let XMLNode::Element(e) = c else { continue };
        if HOLDERS.contains(&tag(e)) {
            continue;
        }
        if SHAPES.contains(&tag(e)) {
            hide(e);
        }
        drop_filter(e, re);
        strip(e, re);
    }
}

fn to_string(root: &Element) -> String {
    let mut buf = vec![];
    let cfg = xmltree::EmitterConfig::new().write_document_declaration(false).perform_indent(false);
    root.write_with_config(&mut buf, cfg).unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

fn cjk_font_missing(root: &Element) -> bool {
    let has_cjk = |s: &str| s.chars().any(|c| matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xff00..=0xffef));
    let mut found = false;
    fn walk(e: &Element, f: &mut bool, t: &dyn Fn(&str) -> bool) {
        if e.name == "text" && t(&itertext(e)) {
            *f = true;
        }
        for c in &e.children {
            if let XMLNode::Element(x) = c {
                walk(x, f, t);
            }
        }
    }
    walk(root, &mut found, &has_cjk);
    if !found {
        return false;
    }
    match std::process::Command::new("fc-list").args([":lang=ja", "family"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        Err(_) => false,
    }
}

pub struct Opts {
    pub out: Option<String>,
    pub width: u32,
    pub small: u32,
    pub min_text: f64,
    pub json: bool,
}

pub fn run(path: &str, o: &Opts) -> Result<bool, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let root = Element::parse(src.as_bytes()).map_err(|e| format!("{path}: {e}"))?;
    let vb: Vec<f64> = Regex::new(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?").unwrap().find_iter(root.attributes.get("viewBox").map_or("", |s| s.as_str())).filter_map(|m| m.as_str().parse().ok()).collect();
    let (x0, y0, w, h) = if vb.len() == 4 {
        (vb[0], vb[1], vb[2], vb[3])
    } else {
        (0.0, 0.0, fnum(root.attributes.get("width").map(|s| s.as_str()), 600.0), fnum(root.attributes.get("height").map(|s| s.as_str()), 600.0))
    };
    let scale = o.width as f64 / w;
    let pad = 0.25 * w.max(h);
    let name = std::path::Path::new(path).file_stem().map_or("svg".into(), |s| s.to_string_lossy().to_string());
    let out = o.out.clone().map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("antialgorithm_check").join(&name));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let opt = options();
    let full_r = render(&src, o.width, &opt)?;
    let small_r = render(&src, o.small, &opt)?;
    let (full_png, small_png) = (out.join("full.png"), out.join("small.png"));
    full_r.pixmap.save_png(&full_png).map_err(|e| e.to_string())?;
    small_r.pixmap.save_png(&small_png).map_err(|e| e.to_string())?;
    let full = flat(&full_r, 255.0);
    let clear = full_r.rgba.chunks_exact(4).any(|p| p[3] < 250.0);
    let full_dark = if clear { Some(flat(&full_r, 0.0)) } else { None };
    let filt_re = Regex::new(r"filter\s*:[^;]*;?").unwrap();

    let mut infos = vec![];
    collect(&root, false, has_filter(&root), &mut infos);
    let n = infos.len();
    let plain = if infos.iter().any(|t| t.own_filter) {
        let mut nf = root.clone();
        each_text(&mut nf, false, &mut |t| drop_filter(t, &filt_re));
        Some(flat(&render(&to_string(&nf), o.width, &opt)?, 255.0))
    } else {
        None
    };

    let mut problems: Vec<String> = vec![];
    if cjk_font_missing(&root) {
        problems.push("Japanese text found but no Japanese font is installed, so it may render as empty boxes (install Noto Sans CJK JP)".into());
    }
    let mut names: Vec<String> = vec![];
    let mut tspans: Vec<usize> = vec![];
    {
        let mut r2 = root.clone();
        each_text(&mut r2, false, &mut |t| {
            names.push(label(t));
            tspans.push(t.children.iter().filter(|c| matches!(c, XMLNode::Element(e) if e.name == "tspan")).count());
        });
    }
    let lum_of = |img: &Img, m: &[bool]| -> f32 {
        let (mut s, mut c) = (0.0, 0usize);
        for (i, &b) in m.iter().enumerate() {
            if b {
                s += lum(&img.rgb, i);
                c += 1;
            }
        }
        if c == 0 { 0.0 } else { s / c as f32 }
    };
    for i in 0..n {
        let mut solo = root.clone();
        strip(&mut solo, &filt_re);
        let mut j = 0;
        each_text(&mut solo, false, &mut |t| {
            if j != i {
                hide(t);
            }
            j += 1;
        });
        solo.attributes.insert("viewBox".into(), format!("{} {} {} {}", x0 - pad, y0 - pad, w + 2.0 * pad, h + 2.0 * pad));
        solo.attributes.insert("width".into(), format!("{}", w + 2.0 * pad));
        solo.attributes.insert("height".into(), format!("{}", h + 2.0 * pad));
        let r = render(&to_string(&solo), ((w + 2.0 * pad) * scale).round() as u32, &opt)?;
        let nm = &names[i];
        let (mut xmin, mut xmax, mut ymin, mut ymax, mut any) = (usize::MAX, 0, usize::MAX, 0, false);
        for y in 0..r.h {
            for x in 0..r.w {
                if r.rgba[(y * r.w + x) * 4 + 3] > 40.0 {
                    any = true;
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                    ymin = ymin.min(y);
                    ymax = ymax.max(y);
                }
            }
        }
        if !any {
            problems.push(format!("text '{nm}': draws nothing"));
            continue;
        }
        let p = (pad * scale).round() as usize;
        let (cw, ch) = (full.w, full.h);
        let mut sides = vec![];
        if xmin < p { sides.push("left"); }
        if xmax >= p + cw { sides.push("right"); }
        if ymin < p { sides.push("top"); }
        if ymax >= p + ch { sides.push("bottom"); }
        if !sides.is_empty() {
            problems.push(format!("text '{nm}': runs off the {} edge", sides.join("/")));
        }
        let lines = tspans[i].max(1) as f64;
        let share = ((ymax - ymin + 1) as f64) / scale / lines / w.max(h);
        if share * 100.0 < o.min_text && sides.is_empty() {
            problems.push(format!("text '{nm}': very small, about {:.1}% of the image side (want {}%+)", share * 100.0, o.min_text));
        }
        let (ih, iw) = ((r.h.saturating_sub(p)).min(ch), (r.w.saturating_sub(p)).min(cw));
        let mut mask = vec![false; cw * ch];
        let mut own = vec![0f32; cw * ch * 3];
        for y in 0..ih {
            for x in 0..iw {
                let si = ((y + p) * r.w + (x + p)) * 4;
                if r.rgba[si + 3] > 200.0 {
                    mask[y * cw + x] = true;
                }
                for c in 0..3 {
                    own[(y * cw + x) * 3 + c] = r.rgba[si + c];
                }
            }
        }
        let count = mask.iter().filter(|b| **b).count();
        if count < 4 {
            continue;
        }
        let base = if infos[i].own_filter { plain.as_ref().unwrap_or(&full) } else { &full };
        let mut cover = 0usize;
        for (k, &m) in mask.iter().enumerate() {
            if m {
                let d = (0..3).map(|c| (base.rgb[k * 3 + c] - own[k * 3 + c]).abs()).sum::<f32>() / 3.0;
                if d > 60.0 {
                    cover += 1;
                }
            }
        }
        let cover = cover as f64 / count as f64;
        let rr = ((3.0 * scale) as i64).max(2);
        let mut ring = vec![false; cw * ch];
        for y in 0..ch as i64 {
            for x in 0..cw as i64 {
                if !mask[y as usize * cw + x as usize] {
                    continue;
                }
                for dy in [-rr, 0, rr] {
                    for dx in [-rr, 0, rr] {
                        let (ny, nx) = (y + dy, x + dx);
                        if ny >= 0 && nx >= 0 && (ny as usize) < ch && (nx as usize) < cw {
                            ring[ny as usize * cw + nx as usize] = true;
                        }
                    }
                }
            }
        }
        for k in 0..ring.len() {
            if mask[k] {
                ring[k] = false;
            }
        }
        let mut contrast = 255.0f32;
        if ring.iter().any(|b| *b) {
            let mut best = 0.0f32;
            for img in [Some(&full), full_dark.as_ref()].into_iter().flatten() {
                best = best.max((lum_of(img, &mask) - lum_of(img, &ring)).abs());
            }
            contrast = best;
        }
        if cover > 0.04 && !infos[i].anc_filtered {
            problems.push(format!("text '{nm}': {:.0}% covered or changed by something drawn over it", cover * 100.0));
        } else if contrast < 30.0 {
            problems.push(format!("text '{nm}': low contrast with what is behind it ({contrast:.0}, want 30+)"));
        }
    }
    if o.json {
        println!("{}", json!({"full": full_png, "small": small_png, "texts_checked": n, "see_through_background": clear, "problems": problems}));
    } else {
        println!("render: {}", full_png.display());
        println!("small {}px: {}", o.small, small_png.display());
        println!("texts checked: {n}");
        if clear {
            println!("background has see-through parts; contrast was measured on white and on black, the better one counts");
        }
        if problems.is_empty() {
            println!("no text problems found. Now look at both renders yourself.");
        } else {
            println!("problems:");
            for pr in &problems {
                println!("  - {pr}");
            }
        }
    }
    Ok(problems.is_empty())
}
