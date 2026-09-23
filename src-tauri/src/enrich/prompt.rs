//! The prompt from SPEC 5.4, wrapped in Qwen3's chat template.

/// Notes longer than this are truncated for enrichment only, head and tail
/// kept, so a long note still gets labelled without blowing the context.
/// Roughly the ~3000 tokens SPEC 5.1 allows, counted in characters.
const MAX_NOTE_CHARS: usize = 12_000;

/// How many of the existing tags to offer the model (SPEC 5.4).
pub const EXISTING_TAG_LIMIT: usize = 150;

pub const SYSTEM: &str = "\
You label short personal notes. Return JSON only.
- subject: a short title, max 8 words, no trailing punctuation.
- summary: one sentence, max 20 words, stating the key point or action.
- tags: 1 to 5 lowercase tags, single words or kebab-case. Tag what the note is
  about: technologies, tools, projects, people, places, topics. When the note
  names a specific project, product, tool, person or place, tag it by that name,
  then add the topics it covers. Prefer the specific name over a broad category
  like tools or software.
  Do not tag what kind of note it is, such as issue, plan, update, sync, task,
  note, question or comparison.
First decide the note's topics from its own words. EXISTING TAGS only tells
you how a topic is already spelled: when one of your topics is there, use that
spelling. Never add a tag just because it is in the list. When none of them fit,
make a new tag rather than forcing a poor match.
Write subject and summary in the same language as the note.";

pub fn user_message(body: &str, existing_tags: &[String]) -> String {
    format!(
        "EXISTING TAGS: {}\n\nNOTE:\n{}",
        existing_tags
            .iter()
            .take(EXISTING_TAG_LIMIT)
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
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

pub fn build(body: &str, existing_tags: &[String]) -> String {
    chat(SYSTEM, &user_message(body, existing_tags))
}

/// Keep the head and the tail: the opening says what a note is about and the
/// end usually carries the decision.
fn truncate(body: &str) -> String {
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
        assert!(SYSTEM.starts_with("You label short personal notes. Return JSON only."));
        assert!(SYSTEM.contains("max 8 words"));
        assert!(SYSTEM.contains("max 20 words"));
        assert!(SYSTEM.contains("1 to 5 lowercase tags"));
        // The two things that keep the vocabulary useful rather than merely
        // small: tag the subject matter, and never force-fit an existing tag.
        assert!(SYSTEM.contains("Tag what the note is"));
        assert!(SYSTEM.contains("Do not tag what"));
        assert!(SYSTEM.contains("tag it by that name"));
        assert!(SYSTEM.contains("rather than forcing a poor match"));
        assert!(SYSTEM.contains("same language as the note"));
    }

    #[test]
    fn the_user_message_lists_existing_tags_then_the_note() {
        let message = user_message("buy a hub", &["argocd".into(), "homelab".into()]);
        assert_eq!(
            message,
            "EXISTING TAGS: argocd, homelab\n\nNOTE:\nbuy a hub"
        );
    }

    #[test]
    fn offers_at_most_the_top_tags() {
        let many: Vec<String> = (0..300).map(|i| format!("tag{i}")).collect();
        let message = user_message("note", &many);
        let listed = message
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("EXISTING TAGS: ")
            .split(", ")
            .count();
        assert_eq!(listed, EXISTING_TAG_LIMIT);
    }

    #[test]
    fn an_empty_vocabulary_still_produces_a_usable_message() {
        let message = user_message("buy a hub", &[]);
        assert!(message.starts_with("EXISTING TAGS: \n\nNOTE:"));
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
