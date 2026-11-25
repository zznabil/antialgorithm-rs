use std::path::PathBuf;
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("aa_test_{}_{}", name, std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(hist: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(args).env("ANTIALGORITHM_HISTORY", hist).output().unwrap()
}

#[test]
fn keep_clash_and_typo_exit_2() {
    let h = tmp("keep").join("h.jsonl");
    let clash = run(&h, &["seed", "visual", "--no-log", "--keep", "method=airport departure board", "--keep", "type=hand lettered"]);
    assert_eq!(clash.status.code(), Some(2));
    let typo = run(&h, &["seed", "visual", "--no-log", "--keep", "method=nonsense"]);
    assert_eq!(typo.status.code(), Some(2));
}

#[test]
fn rolls_never_hold_a_banned_pair() {
    let h = tmp("roll").join("h.jsonl");
    for _ in 0..150 {
        let o = run(&h, &["seed", "visual", "--no-log", "--json"]);
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        let (m, t) = (v["method"].as_str().unwrap(), v["type"].as_str().unwrap());
        assert!(!(m == "airport departure board" && t == "hand lettered"));
        if ["LED dot matrix", "thermal receipt printer", "x-ray image"].contains(&m) {
            assert_eq!(v["color"], "the method's own colors");
        }
    }
}

#[test]
fn parallel_moves_all_kept_and_caps_hold() {
    let h = tmp("race").join("h.jsonl");
    let kids: Vec<_> = (0..20).map(|i| {
        Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(["moves", "add", &format!("race {i}")]).env("ANTIALGORITHM_HISTORY", &h).spawn().unwrap()
    }).collect();
    for mut k in kids { assert!(k.wait().unwrap().success()); }
    assert_eq!(std::fs::read_to_string(&h).unwrap().lines().count(), 20);
    for _ in 0..65 { run(&h, &["seed", "idea"]); }
    let rows = std::fs::read_to_string(&h).unwrap();
    let seeds = rows.lines().filter(|l| l.contains("\"type\":\"seed\"")).count();
    let moves = rows.lines().filter(|l| l.contains("\"type\":\"move\"")).count();
    assert_eq!((seeds, moves), (60, 20));
}

#[test]
fn only_reroll_is_not_logged() {
    let h = tmp("only").join("h.jsonl");
    let o = run(&h, &["seed", "visual", "--only", "color"]);
    assert!(o.status.success());
    assert!(!h.exists());
}

#[test]
fn check_exit_codes_and_findings() {
    let d = tmp("check");
    let bad = d.join("bad.svg");
    std::fs::write(&bad, r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 600"><rect width="400" height="600" fill="#102030"/><rect x="50" y="200" width="300" height="120" fill="#ffcc00"/><text x="60" y="100" font-family="Arial" font-size="40" fill="#102030">LOW</text><text x="-40" y="500" font-family="Arial" font-size="60" fill="#fff">OFFEDGE</text><text x="60" y="260" font-family="Arial" font-size="30" fill="#fff">fine</text><rect x="40" y="380" width="200" height="60" fill="#fff"/><text x="60" y="420" font-family="Arial" font-size="30" fill="#000">buried</text><rect x="40" y="380" width="200" height="60" fill="#888"/></svg>"##).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(["check", bad.to_str().unwrap(), "--out", d.join("o").to_str().unwrap(), "--json"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let p = v["problems"].to_string();
    assert!(p.contains("LOW") && p.contains("OFFEDGE") && p.contains("buried"), "{p}");
    let good = d.join("good.svg");
    std::fs::write(&good, r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 600"><rect width="400" height="600" fill="#102030"/><text x="40" y="300" font-family="Arial" font-size="40" fill="#ffffff">fine</text></svg>"##).unwrap();
    let g = Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(["check", good.to_str().unwrap(), "--out", d.join("g").to_str().unwrap()]).output().unwrap();
    assert_eq!(g.status.code(), Some(0), "{}", String::from_utf8_lossy(&g.stdout));
}

#[test]
fn rough_bleeds_past_edges_and_is_seed_stable() {
    let d = tmp("rough");
    let src = d.join("a.svg");
    std::fs::write(&src, r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 600"><rect x="0" y="0" width="200" height="100" fill="red"/><line x1="10" y1="10" x2="390" y2="300" stroke="#000"/><text x="20" y="400" font-size="48">HI</text></svg>"##).unwrap();
    let (a, b) = (d.join("a1.svg"), d.join("a2.svg"));
    for out in [&a, &b] {
        let o = Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(["rough", src.to_str().unwrap(), out.to_str().unwrap(), "--seed", "7"]).output().unwrap();
        assert!(o.status.success());
    }
    let s = std::fs::read_to_string(&a).unwrap();
    assert_eq!(s, std::fs::read_to_string(&b).unwrap());
    assert!(s.contains("<path") && !s.contains("<line") && s.contains("rough-edge"));
    let g = Command::new(env!("CARGO_BIN_EXE_antialgorithm")).args(["check", a.to_str().unwrap(), "--out", d.join("o").to_str().unwrap()]).output().unwrap();
    assert!(g.status.code().is_some());
}
