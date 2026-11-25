mod check;
mod data;
mod history;
mod rough;
mod seed;

use clap::{Parser, Subcommand};
use serde_json::json;
use std::process::ExitCode;

const SKILL: &str = include_str!("../SKILL.md");
const VISUALS: &str = include_str!("../visuals.md");

#[derive(Parser)]
#[command(name = "antialgorithm", version, about = "Seed, move log, SVG roughening and SVG text checks for the antialgorithm skill.\nExit codes: 0 ok, 1 check found problems, 2 bad input.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Roll a random starting point.
    Seed {
        #[arg(default_value = "visual", value_parser = ["visual", "text", "idea"])]
        kind: String,
        /// Roll one axis again.
        #[arg(long)]
        only: Option<String>,
        /// Hold an axis: axis=value. Repeat for more.
        #[arg(long)]
        keep: Vec<String>,
        /// Do not save this seed to the history.
        #[arg(long)]
        no_log: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show or add the main move of an answer.
    Moves {
        #[arg(long, default_value_t = 12)]
        last: usize,
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<MovesAction>,
    },
    /// Break the straight lines and even spacing of an SVG made by hand methods.
    Rough {
        src: String,
        dst: String,
        #[arg(long, default_value_t = 1.0)]
        amount: f64,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Render an SVG and list text that runs off an edge, is covered, has low contrast or is too small.
    Check {
        svg: String,
        #[arg(long)]
        out: Option<String>,
        #[arg(long, default_value_t = 800)]
        width: u32,
        #[arg(long, default_value_t = 64)]
        small: u32,
        /// Smallest text height as a percent of the image side; about 0.6 for interfaces.
        #[arg(long, default_value_t = 1.2)]
        min_text: f64,
        #[arg(long)]
        json: bool,
    },
    /// Print the skill text built into this binary.
    Skill {
        #[arg(default_value = "skill", value_parser = ["skill", "visuals"])]
        part: String,
    },
}

#[derive(Subcommand)]
enum MovesAction {
    /// Save a move. Name the move, not the project.
    Add { text: Vec<String> },
}

fn run(cli: Cli) -> Result<bool, String> {
    match cli.cmd {
        Cmd::Seed { kind, only, keep, no_log, json } => seed::run(&kind, &keep, only, no_log, json).map(|_| true),
        Cmd::Moves { last, json, action } => {
            if let Some(MovesAction::Add { text }) = action {
                let t = text.join(" ").trim().to_string();
                if t.is_empty() {
                    return Err("add needs the move, for example: antialgorithm moves add \"poster: x-ray method, deep blue ground\"".into());
                }
                history::add(json!({"type": "move", "move": t})).map_err(|e| e.to_string())?;
                println!("saved: {t}");
                return Ok(true);
            }
            let rows = history::load(Some("move"));
            let rows: Vec<_> = rows[rows.len().saturating_sub(last)..].iter().rev().collect();
            if json {
                println!("{}", serde_json::to_string(&rows).unwrap());
            } else if rows.is_empty() {
                println!("no moves saved yet");
            } else {
                for r in rows {
                    println!("{}  {}", r.get("date").and_then(|d| d.as_str()).unwrap_or("?"), r.get("move").and_then(|d| d.as_str()).unwrap_or(""));
                }
            }
            Ok(true)
        }
        Cmd::Rough { src, dst, amount, seed } => rough::run(&src, &dst, amount, seed).map(|_| true),
        Cmd::Check { svg, out, width, small, min_text, json } => check::run(&svg, &check::Opts { out, width, small, min_text, json }),
        Cmd::Skill { part } => {
            print!("{}", if part == "visuals" { VISUALS } else { SKILL });
            Ok(true)
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
