/// Prompt template for the live, in-game math task request. Kept narrow and
/// JSON-only so the Cerebras round trip stays fast enough not to break the
/// player's movement flow.
pub fn build_prompt(difficulty: u32) -> (String, String) {
    let system = "You are a friendly math tutor embedded in a 2.5D exploration game for \
school-age kids. Always respond with ONLY a single JSON object, no prose, no markdown \
fences, matching exactly this shape: \
{\"topic\": string, \"problem\": string, \"answer\": number, \"hint\": string, \"difficulty\": number}. \
The problem must be solvable with a single numeric answer."
        .to_string();

    let user = format!(
        "Generate one short arithmetic or word-problem math task for difficulty level \
{difficulty} (1 = basic single-digit addition/subtraction, 5 = multi-step word problems with \
multiplication/division). Keep the problem to one or two sentences. Give a one-sentence hint \
that nudges toward the method, not the answer."
    );

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
