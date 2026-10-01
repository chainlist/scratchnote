//! The prompt from SPEC 5.4, wrapped in Qwen3's chat template.

/// Notes longer than this are truncated for enrichment only, head and tail
/// kept, so a long note still gets labelled without blowing the context.
/// Roughly the ~3000 tokens SPEC 5.1 allows, counted in characters.
const MAX_NOTE_CHARS: usize = 12_000;

/// `language` is the English name of the one the user picked, such as
/// "French": the subject follows it whatever language the note is in.
pub fn system(language: &str) -> String {
    format!(
        "\
You label short personal notes. Return JSON only.
Write the subject in {language}, whatever language the note is in: a note in
another language gets a subject in {language}, translated. Names of projects,
products, tools, people and places stay as they are.
- subject: a title of 3 to 8 words naming what the note is about. Do not copy
  the note's first sentence. No trailing punctuation.
- category: the broad subject the note belongs to, such as its field,
  technology, hobby or medium, so it sits next to related notes that name
  other things. Not what kind of note it is. Pick it from CATEGORIES, spelled
  exactly as listed. Leave it empty only when none of them is the subject of
  the note."
    )
}

/// The language is named again after the note: said only in the system
/// message, the 4B labels an English note in English whatever it asks.
pub fn user_message(body: &str, language: &str, categories: &[String]) -> String {
    format!(
        "CATEGORIES: {}\n\nNOTE:\n{}\n\nLABEL IN: {language}.",
        categories.join(", "),
        truncate(body)
    )
}

/// Qwen3 chat template. The empty think block is how thinking is turned off
/// for the hybrid models; the Instruct variants ignore it harmlessly.
pub fn chat(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n\
         <|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n<think>\n\n</think>\n\n"
    )
}

pub fn build(body: &str, language: &str, categories: &[String]) -> String {
    chat(&system(language), &user_message(body, language, categories))
}

/// Keep the head and the tail: the opening says what a note is about and the
/// end usually carries the decision.
pub(crate) fn truncate(body: &str) -> String {
    if body.chars().count() <= MAX_NOTE_CHARS {
        return body.to_string();
    }
    let half = MAX_NOTE_CHARS / 2;
    let head: String = body.chars().take(half).collect();
    let tail: String = body
        .chars()
        .skip(body.chars().count().saturating_sub(half))
        .collect();
    format!("{head}\n[...]\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_message_matches_the_spec() {
        let system = system("French");
        assert!(system.starts_with("You label short personal notes. Return JSON only."));
        assert!(system.contains("3 to 8 words"));
        assert!(system.contains("- category:"));
        assert!(system.contains("Pick it from CATEGORIES"));
        assert!(system.contains("what kind of note it is"));
        assert!(system.contains("subject in French, translated"));
        assert!(!system.contains("tags"));
    }

    #[test]
    fn the_user_message_lists_the_categories_then_the_note_and_language() {
        let message = user_message("buy a hub", "French", &["hardware".into(), "maison".into()]);
        assert_eq!(
            message,
            "CATEGORIES: hardware, maison\n\nNOTE:\nbuy a hub\n\nLABEL IN: French."
        );
    }

    #[test]
    fn disables_thinking_in_the_chat_template() {
        let prompt = chat("sys", "usr");
        assert!(prompt.contains("<|im_start|>system\nsys<|im_end|>"));
        assert!(prompt.contains("<|im_start|>user\nusr<|im_end|>"));
        assert!(prompt.ends_with("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn long_notes_keep_their_head_and_tail() {
        let body = format!("{}MIDDLE{}", "h".repeat(20_000), "t".repeat(20_000));
        let out = truncate(&body);
        assert!(out.starts_with("hhh"));
        assert!(out.ends_with("ttt"));
        assert!(out.contains("[...]"));
        assert!(!out.contains("MIDDLE"));
        assert!(out.chars().count() < body.chars().count());
    }

    #[test]
    fn short_notes_are_passed_through_untouched() {
        assert_eq!(truncate("a short note"), "a short note");
    }
}
