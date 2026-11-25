use fs2::FileExt;
use serde_json::{json, Map, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LIMIT: usize = 300;

fn limit(kind: &str) -> usize {
    match kind {
        "seed" => 60,
        "move" => 300,
        _ => DEFAULT_LIMIT,
    }
}

pub fn path() -> PathBuf {
    if let Ok(p) = std::env::var("ANTIALGORITHM_HISTORY") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let state = match std::env::var("XDG_STATE_HOME") {
        Ok(s) if !s.is_empty() => PathBuf::from(s),
        _ => {
            let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".local").join("state")
        }
    };
    state.join("antialgorithm").join("history.jsonl")
}

pub fn load(kind: Option<&str>) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(path()) else { return vec![] };
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| kind.map_or(true, |k| e.get("type").and_then(Value::as_str) == Some(k)))
        .collect()
}

fn today() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

fn trim(rows: Vec<Value>) -> Vec<Value> {
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    let mut kept: Vec<Value> = Vec::new();
    for row in rows.into_iter().rev() {
        let kind = row.get("type").and_then(Value::as_str).unwrap_or("").to_string();
        let c = counts.entry(kind.clone()).or_insert(0);
        *c += 1;
        if *c <= limit(&kind) {
            kept.push(row);
        }
    }
    kept.reverse();
    kept
}

pub fn add(entry: Value) -> std::io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let lock_path = p.with_extension("jsonl.lock");
    let lock = OpenOptions::new().create(true).write(true).open(&lock_path)?;
    lock.lock_exclusive()?;
    let mut row = Map::new();
    row.insert("date".into(), json!(today()));
    if let Value::Object(o) = entry {
        row.extend(o);
    }
    let mut rows = load(None);
    rows.push(Value::Object(row));
    let rows = trim(rows);
    let tmp = p.with_extension("jsonl.tmp");
    {
        let mut f: File = File::create(&tmp)?;
        for r in &rows {
            writeln!(f, "{}", serde_json::to_string(r).unwrap())?;
        }
    }
    fs::rename(&tmp, &p)?;
    lock.unlock()?;
    Ok(())
}
