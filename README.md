# antialgorithm-rs

One static binary for the [antialgorithm](https://github.com/mankidev/antialgorithm) skill. Replaces its Python scripts. No runtime. No numpy, Pillow or rsvg-convert.

| Command | Job |
|---|---|
| `antialgorithm seed visual\|text\|idea` | Roll a random start point. Avoids recent values. |
| `antialgorithm moves` / `moves add "text"` | List or log the main move of an answer. |
| `antialgorithm rough in.svg out.svg` | Break straight lines and even spacing of a hand-made SVG. |
| `antialgorithm check file.svg` | Find text that runs off an edge, is covered, has low contrast or is too small. |
| `antialgorithm skill [visuals]` | Print the skill text built into the binary. |

## Install

Download the file for your system from [Releases](https://github.com/zznabil/antialgorithm-rs/releases), rename it to `antialgorithm` (`antialgorithm.exe` on Windows), put it on `PATH`.

Or build it:

```
cargo install --git https://github.com/zznabil/antialgorithm-rs
```

## Use with an agent

1. Install the binary.
2. Run `antialgorithm skill` and save the output as the skill file, or copy `SKILL.md` and `visuals.md` from this repo.
3. The agent calls the commands. `--json` on `seed`, `moves` and `check` gives machine output.

Exit codes: `0` ok, `1` `check` found problems, `2` bad input.

## Reference

- History file: `~/.local/state/antialgorithm/history.jsonl`. Set `$XDG_STATE_HOME` or `$ANTIALGORITHM_HISTORY` to move it. Caps: 60 seeds, 300 moves. Parallel writes are safe.
- Seed lists, tags and clash rules: `data/seed.json`. One source. Embedded at build time.
- `rough --seed N` repeats the result. Output differs from the Python version, which used another random generator.
- Dates in history are UTC.

## Develop

```
cargo test --release
```

## Credit

Skill text and design: [mankidev/antialgorithm](https://github.com/mankidev/antialgorithm), MIT. This repo is a Rust port with edits to the seed lists and history.
