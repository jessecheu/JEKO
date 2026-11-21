//! Offline batch generator. Run with `cargo run --bin gen_content`
//! whenever you want fresh content; the shipped game never calls this.
//!
//! - no arguments: calls Cerebras once per NPC to write a bank of
//!   flavor-text dialogue lines to assets/dialogue.json, after
//!   `dialogue::check_bank` accepts it (a rejected bank leaves the old file).
//! - `--monsters N [--dir PATH]`: has the monster studio's code path (FR21)
//!   design N monsters, cycling through the four tiers, and writes them to
//!   `PATH/monsters/gen-<name>.json` (default: the content dir). Review
//!   them, then rename the ones worth keeping (drop the `gen-` prefix and
//!   fix the `id`) and commit them.

use jeko::config::Config;
use jeko::content::prompts::TOPICS;
use jeko::content::studio::{generate_blocking, MonsterJob};
use jeko::content::{self, monster_spec};
use jeko::llm::{cerebras_client::CerebrasClient, task_gen};
use serde_json::json;
use std::fs;

const NPC_NAMES: &[&str] = &["Professor Pebble", "Wanderer Finch", "Old Lantern"];

fn main() -> anyhow::Result<()> {
    let cfg = Config::load();
    if cfg.api_key.is_none() {
        anyhow::bail!("set CEREBRAS_API_KEY (see .env.example) before running gen_content");
    }
    let client = CerebrasClient::new(&cfg);

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(at) = args.iter().position(|a| a == "--monsters") {
        let n: usize = args
            .get(at + 1)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("usage: gen_content --monsters N [--dir PATH]"))?;
        let dir = args
            .iter()
            .position(|a| a == "--dir")
            .and_then(|i| args.get(i + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(content::content_dir);
        return gen_monsters(&client, &dir, n);
    }

    let system = "You write short, warm flavor-text lines (one sentence each) spoken by NPCs in \
a 2.5D educational exploration game for kids learning math. Output ONLY a JSON array of strings, \
no markdown fences, no extra prose.";

    let mut npcs = Vec::new();
    for name in NPC_NAMES {
        let user = format!(
            "Write 5 distinct one-sentence lines of dialogue for an NPC named \"{name}\" who \
encourages a student exploring a math-themed overworld. Keep it upbeat and age-appropriate."
        );

        let raw = client.request_raw_blocking(system, &user)?;
        let json_slice = task_gen::extract_json_array(&raw)
            .ok_or_else(|| anyhow::anyhow!("no JSON array in response: {raw}"))?;
        let lines: Vec<String> = serde_json::from_str(json_slice)?;

        println!("generated {} lines for {name}", lines.len());
        npcs.push(json!({ "npc": name, "lines": lines }));
    }

    // Check the whole bank before it replaces the shipped one: the game
    // compiles this file in, and a bad one would fail the build or the start.
    let json = serde_json::to_string_pretty(&npcs)?;
    jeko::game::dialogue::check_bank(&json)
        .map_err(|e| anyhow::anyhow!("not writing assets/dialogue.json: {e}"))?;
    fs::write("assets/dialogue.json", json)?;
    println!("wrote assets/dialogue.json");
    Ok(())
}

/// Designs `n` monsters one after another, tiers 1 to 4 in turn, telling the
/// model the names already taken so it doesn't repeat them.
fn gen_monsters(client: &CerebrasClient, dir: &std::path::Path, n: usize) -> anyhow::Result<()> {
    let mut taken: Vec<String> = Vec::new();
    let mut made = 0;
    for i in 0..n {
        let tier = (i % 4) as u32 + 1;
        let topic = TOPICS[((tier as usize - 1) * 2 + i / 4 % 2) % TOPICS.len()];
        let job = MonsterJob {
            tier,
            topic: topic.to_string(),
            avoid: taken.clone(),
        };
        match generate_blocking(client, dir, &job) {
            Ok(id) => {
                let path = dir.join("monsters").join(format!("{id}.json"));
                let spec = monster_spec::parse(&std::fs::read_to_string(&path)?)
                    .map_err(|e| anyhow::anyhow!("{id}: {e}"))?;
                println!("tier {tier}  {id}  \"{}\"  {}", spec.name, spec.blurb);
                taken.push(spec.name);
                made += 1;
            }
            Err(e) => eprintln!("tier {tier}  {e}"),
        }
    }
    println!(
        "wrote {made} of {n} monsters under {}",
        dir.join("monsters").display()
    );
    Ok(())
}
