//! Offline batch generator: calls Cerebras once per NPC to write a bank of
//! flavor-text dialogue lines to assets/dialogue.json. Run with
//! `cargo run --bin gen_content` whenever you want fresh NPC lines; the
//! shipped game never calls this at runtime.

use jeko::config::Config;
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
