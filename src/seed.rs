use crate::data::data;
use crate::history;
use rand::rngs::OsRng;
use rand::Rng;
use serde_json::json;
use std::collections::HashMap;

pub type Seed = Vec<(String, String)>;
const OWN_COLORS: &str = "the method's own colors";

fn choose<'a>(items: &'a [String]) -> &'a String {
    &items[OsRng.gen_range(0..items.len())]
}

struct Recent(HashMap<String, Vec<String>>);

impl Recent {
    fn load(kind: &str, seeds: usize) -> Self {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        let same: Vec<_> = history::load(Some("seed"))
            .into_iter()
            .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some(kind))
            .collect();
        let start = same.len().saturating_sub(seeds);
        for e in same[start..].iter().rev() {
            if let Some(obj) = e.get("seed").and_then(|s| s.as_object()) {
                for (axis, v) in obj {
                    if let Some(v) = v.as_str() {
                        let list = map.entry(axis.clone()).or_default();
                        if !list.iter().any(|x| x == v) {
                            list.push(v.to_string());
                        }
                    }
                }
            }
        }
        Recent(map)
    }

    fn pick(&self, axis: &str, items: &[String]) -> String {
        let n = items.len() / 2;
        let avoid: &[String] = self.0.get(axis).map(|v| &v[..v.len().min(n)]).unwrap_or(&[]);
        let fresh: Vec<String> = items.iter().filter(|i| !avoid.contains(i)).cloned().collect();
        if fresh.is_empty() { choose(items).clone() } else { choose(&fresh).clone() }
    }
}

fn get<'a>(s: &'a Seed, k: &str) -> Option<&'a str> {
    s.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

fn set(s: &mut Seed, k: &str, v: String) {
    if let Some(e) = s.iter_mut().find(|(a, _)| a == k) {
        e.1 = v;
    } else {
        s.push((k.to_string(), v));
    }
}

pub fn clashes(seed: &Seed) -> bool {
    let d = data();
    for [a, b] in &d.clash {
        let hit = |m: &std::collections::BTreeMap<String, String>| m.iter().all(|(k, v)| get(seed, k) == Some(v.as_str()));
        if hit(a) && hit(b) {
            return true;
        }
    }
    let Some(m) = get(seed, "method") else { return false };
    let has = |v: &Vec<String>, x: &str| v.iter().any(|y| y == x);
    if let Some(t) = get(seed, "type") {
        if (has(&d.machine_methods, m) && has(&d.hand_types, t)) || (has(&d.hand_methods, m) && has(&d.machine_types, t)) {
            return true;
        }
    }
    if has(&d.fixed_color, m) {
        return get(seed, "color") != Some(OWN_COLORS);
    }
    if get(seed, "color") == Some(OWN_COLORS) {
        return true;
    }
    let family = d.methods.iter().find(|(_, v)| has(v, m)).map(|(k, _)| k.as_str());
    get(seed, "color") == Some("the colors of a machine screen") && family != Some("screen and machine")
}

fn pools(kind: &str) -> Vec<(&'static str, Option<Vec<String>>)> {
    let d = data();
    match kind {
        "visual" => vec![
            ("method", None),
            ("color", Some(d.visual.color.clone())),
            ("layout", Some(d.visual.layout.clone())),
            ("type", Some(d.visual.r#type.clone())),
        ],
        "text" => vec![
            ("voice", Some(d.text.voice.clone())),
            ("entry", Some(d.text.entry.clone())),
            ("register", Some(d.text.register.clone())),
        ],
        _ => vec![
            ("verb", Some(d.idea.verb.clone())),
            ("constraint", Some(d.idea.constraint.clone())),
            ("scale", Some(d.idea.scale.clone())),
            ("pressure", Some(d.idea.pressure.clone())),
        ],
    }
}

fn roll_once(kind: &str, keep: &Seed, only: Option<&str>, recent: &Recent) -> Seed {
    let d = data();
    let mut out: Seed = vec![];
    for (name, items) in pools(kind) {
        let v = match items {
            None => {
                let fams: Vec<&String> = d.methods.keys().collect();
                let fam = fams[OsRng.gen_range(0..fams.len())];
                recent.pick("method", &d.methods[fam])
            }
            Some(items) => recent.pick(name, &items),
        };
        out.push((name.to_string(), v));
    }
    if kind == "visual" {
        out.push(("verb".into(), recent.pick("verb", &d.idea.verb)));
        out.push(("scale".into(), recent.pick("scale", &d.idea.scale)));
    }
    out.push(("setting".into(), recent.pick("setting", &d.setting)));
    out.push(("region".into(), recent.pick("region", &d.region)));
    let anchor = if kind == "visual" && OsRng.gen_bool(0.5) {
        recent.pick("anchor", &d.no_anchor)
    } else {
        recent.pick("anchor", &d.anchor)
    };
    out.push(("anchor".into(), anchor));
    for (k, v) in keep {
        set(&mut out, k, v.clone());
    }
    if let Some(o) = only {
        out.retain(|(k, _)| keep.iter().any(|(a, _)| a == k) || k == o);
    }
    let keeps_color = keep.iter().any(|(k, _)| k == "color");
    if let Some(m) = get(&out, "method") {
        if d.fixed_color.iter().any(|x| x == m) && get(&out, "color").is_some() && !keeps_color {
            set(&mut out, "color", OWN_COLORS.into());
        }
    }
    out
}

pub fn axes_of(kind: &str) -> Vec<String> {
    roll_once(kind, &vec![], None, &Recent(HashMap::new())).into_iter().map(|(k, _)| k).collect()
}

pub fn allowed(kind: &str) -> HashMap<String, Vec<String>> {
    let d = data();
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for (name, items) in pools(kind) {
        let v = items.unwrap_or_else(|| d.methods.values().flatten().cloned().collect());
        m.insert(name.to_string(), v);
    }
    if kind == "visual" {
        m.get_mut("color").unwrap().push(OWN_COLORS.into());
        m.insert("verb".into(), d.idea.verb.clone());
        m.insert("scale".into(), d.idea.scale.clone());
    }
    m.insert("setting".into(), d.setting.clone());
    m.insert("region".into(), d.region.clone());
    let mut a = d.anchor.clone();
    if kind == "visual" {
        a.extend(d.no_anchor.clone());
    }
    m.insert("anchor".into(), a);
    m
}

pub fn run(kind: &str, keep_args: &[String], only: Option<String>, no_log: bool, as_json: bool) -> Result<(), String> {
    let mut keep: Seed = vec![];
    for item in keep_args {
        let (k, v) = item.split_once('=').ok_or_else(|| format!("--keep needs axis=value, got '{item}'"))?;
        keep.push((k.trim().into(), v.trim().into()));
    }
    let axes = axes_of(kind);
    for name in keep.iter().map(|(k, _)| k.as_str()).chain(only.as_deref()) {
        if !axes.iter().any(|a| a == name) {
            return Err(format!("no axis '{name}' for {kind}; axes: {}", axes.join(", ")));
        }
    }
    let valid = allowed(kind);
    for (k, v) in &keep {
        if !valid[k].contains(v) {
            return Err(format!("'{v}' is not a value of axis '{k}'; values: {}", valid[k].join("; ")));
        }
    }
    if let Some(o) = &only {
        keep.retain(|(k, _)| k != o);
    }
    let recent = Recent::load(kind, 12);
    let mut seed = None;
    for _ in 0..200 {
        let s = roll_once(kind, &keep, only.as_deref(), &recent);
        if !clashes(&s) {
            seed = Some(s);
            break;
        }
    }
    let mut seed = seed.ok_or("the kept axes clash with each other; keep fewer")?;
    if let Some(o) = &only {
        seed.retain(|(k, _)| k == o);
    }
    if as_json {
        let m: serde_json::Map<String, serde_json::Value> = seed.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        println!("{}", serde_json::Value::Object(m));
    } else {
        for (k, v) in &seed {
            println!("{k}: {v}");
        }
    }
    if !no_log && only.is_none() {
        let m: serde_json::Map<String, serde_json::Value> = seed.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        history::add(json!({"type": "seed", "kind": kind, "seed": m})).map_err(|e| e.to_string())?;
    }
    Ok(())
}
