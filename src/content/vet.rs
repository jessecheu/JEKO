//! A word blocklist for text a person or a model writes for the game: NPC lines
//! and, later, monster text. Plain Rust, no macroquad or egui.

/// Words the game's text may not use: other people's characters, because the
/// prompts ask for original creatures, and anything that isn't for a children's
/// game. Matched on whole words, ignoring case. A blocklist is a floor, not a
/// guarantee: the kid-safe prompt and a person's read-through do the main work.
const BLOCKED: &[&str] = &[
    // Characters and brands.
    "pokemon",
    "pikachu",
    "digimon",
    "mario",
    "luigi",
    "bowser",
    "goomba",
    "sonic",
    "zelda",
    "kirby",
    "mickey",
    "disney",
    "pixar",
    "shrek",
    "minecraft",
    "creeper",
    "nintendo",
    "marvel",
    "pacman",
    "yoshi",
    "totoro",
    "godzilla",
    // Not for a children's game.
    "kill",
    "kills",
    "killed",
    "murder",
    "blood",
    "bloody",
    "gore",
    "corpse",
    "dead",
    "death",
    "suicide",
    "gun",
    "guns",
    "rifle",
    "bomb",
    "knife",
    "sword",
    "weapon",
    "drug",
    "drugs",
    "beer",
    "wine",
    "alcohol",
    "sex",
    "sexy",
    "naked",
    "damn",
    "hell",
    "stupid",
    "idiot",
];

/// `Err(word)` if `text` uses a blocked word.
pub fn vet_text(text: &str) -> Result<(), String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .find(|w| BLOCKED.contains(&w.as_str()))
        .map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vet_text_matches_whole_words_only() {
        assert!(vet_text("A kind little frog").is_ok());
        assert!(vet_text("Shellfish and Hellebore").is_ok(), "not 'hell'");
        assert!(vet_text("Bloodroot").is_ok(), "not 'blood'");
        assert!(vet_text("It's Mario!").is_err());
    }
}
