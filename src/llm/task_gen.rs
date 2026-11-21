/// Prompt template for the live, in-game math task request. Kept narrow and
/// JSON-only so the Cerebras round trip stays fast enough not to break the
/// player's movement flow. `topic` themes the problem (a monster's `theme`,
/// FR22); the answer is always a single number.
///
/// The model also writes the sum behind the answer as `expression`, which
/// `llm::expr::verify` checks before a child sees the question, and a short
/// worked `explanation` for the wrong-answer card.
pub fn build_prompt(difficulty: u32, topic: Option<&str>) -> (String, String) {
    let system = "You are a friendly math tutor embedded in a 2.5D exploration game for \
school-age kids. Always respond with ONLY a single JSON object, no prose, no markdown \
fences, matching exactly this shape: \
{\"topic\": string, \"problem\": string, \"answer\": number, \"expression\": string, \
\"hint\": string, \"explanation\": [string], \"difficulty\": number}. \
The problem must be solvable with a single numeric answer. \"expression\" is the arithmetic \
that gives the answer, written with only numbers, + - * / and parentheses, for example \
\"(12 + 8) / 4\". \"explanation\" is 2 to 4 short steps that show how to get the answer."
        .to_string();

    let mut user = format!(
        "Generate one short arithmetic or word-problem math task for difficulty level \
{difficulty} (1 = basic single-digit addition/subtraction, 5 = multi-step word problems with \
multiplication/division). Keep the problem to one or two sentences. Give a one-sentence hint \
that nudges toward the method, not the answer."
    );
    if let Some(topic) = topic {
        user.push_str(&format!(
            " The problem must be about {topic}, and \"topic\" must be \"{topic}\"."
        ));
    }

    (system, user)
}

/// The prompt for a pack of `n` problems on one topic and level (FR22's
/// lesson packs). `like` asks for problems similar to one the player missed:
/// the same idea with different numbers and a different story. Returns
/// `(system, user)`.
pub fn build_pack_prompt(
    topic: &str,
    level: u32,
    n: usize,
    like: Option<&str>,
) -> (String, String) {
    let system = "You write math practice problems for school-age kids (ages 7 to 11) in an \
exploration game. Always respond with ONLY a single JSON array, no prose, no markdown fences. \
Each element has exactly this shape: {\"problem\": string, \"answer\": number, \
\"expression\": string, \"hint\": string, \"explanation\": [string]}. Every problem has a \
single numeric answer. \"expression\" is the arithmetic that gives the answer, written with \
only numbers, + - * / and parentheses, for example \"(12 + 8) / 4\", and it MUST equal \
\"answer\". \"hint\" is one sentence that nudges toward the method without giving the answer. \
\"explanation\" is 2 to 4 short steps that show how to get the answer. Keep every problem to \
one or two sentences, kind and age-appropriate, and different from the others."
        .to_string();
    let mut user = format!(
        "Write {n} problems about {topic} at difficulty level {level} (1 = basic single-digit \
addition/subtraction, 5 = multi-step word problems with multiplication/division)."
    );
    if let Some(missed) = like {
        user.push_str(&format!(
            " The player just got this one wrong, so make them similar practice: the same idea \
with different numbers and a different story, not the same problem: \"{missed}\""
        ));
    }
    (system, user)
}

pub fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

pub fn extract_json_array(raw: &str) -> Option<&str> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    (end > start).then(|| &raw[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_prompt_asks_for_a_checkable_answer_and_the_topic_when_given() {
        let (system, user) = build_prompt(3, None);
        assert!(system.contains("\"expression\"") && system.contains("\"explanation\""));
        assert!(user.contains("level 3") && !user.contains("must be about"));
        let (_, themed) = build_prompt(2, Some("fractions"));
        assert!(themed.contains("about fractions"));
    }

    #[test]
    fn the_pack_prompt_names_the_count_topic_level_and_what_was_missed() {
        let (system, user) = build_pack_prompt("fractions", 2, 8, None);
        assert!(system.contains("JSON array") && system.contains("MUST equal"));
        assert!(user.contains("8 problems about fractions at difficulty level 2"));
        assert!(!user.contains("just got this one wrong"));
        let (_, like) = build_pack_prompt("fractions", 2, 8, Some("What is 1/2 of 8?"));
        assert!(like.contains("What is 1/2 of 8?") && like.contains("different numbers"));
    }

    #[test]
    fn json_is_pulled_out_of_chatty_replies() {
        assert_eq!(
            extract_json_object("Sure! {\"a\": 1} Hope that helps"),
            Some("{\"a\": 1}")
        );
        assert_eq!(extract_json_array("```json\n[1, 2]\n```"), Some("[1, 2]"));
        assert_eq!(extract_json_object("no braces"), None);
        assert_eq!(extract_json_array("] backwards ["), None);
    }
}
