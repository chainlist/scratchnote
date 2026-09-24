//! The prompt from SPEC 5.4, wrapped in Qwen3's chat template.

/// Notes longer than this are truncated for enrichment only, head and tail
/// kept, so a long note still gets labelled without blowing the context.
/// Roughly the ~3000 tokens SPEC 5.1 allows, counted in characters.
const MAX_NOTE_CHARS: usize = 12_000;

/// How many of the existing tags to offer the model (SPEC 5.4).
pub const EXISTING_TAG_LIMIT: usize = 150;

pub const SYSTEM: &str = "\
You label short personal notes. Return JSON only.
Write subject and summary in the same language as the note: a French note gets
a French subject and summary.
- subject: a title of 3 to 8 words naming what the note is about. Do not copy
  the note's first sentence. No trailing punctuation.
- summary: one sentence, max 20 words, stating the key point, decision or next
  action.
- category: one lowercase word or kebab-case naming the broad subject the note
  belongs to, such as its field, technology, hobby or medium, so it sits next
  to related notes that name other things. Not what kind of note it is.
  Pick it from EXISTING CATEGORIES, spelled exactly as listed. Only when none
  of them is the subject of the note, make a new one.
- tags: 1 to 5 lowercase tags, single words or kebab-case. A tag is a shelf the
  user browses later: keep one only if they would want to see other notes
  under it.
  First, when the note names a specific project, product, tool, game, film,
  show, person or place, tag it by that name. A project or product the note
  names is always a tag, even when the note is about one feature of it. Then
  add at most two main subjects it covers. Do not repeat the category.
  Never tag a phrase lifted from the note, a version number, a small detail, or
  what kind of note it is, such as issue, plan, update, task, idea, todo or
  question.
EXISTING TAGS only tells you how a subject is already spelled. Reuse one only
when it means the same thing in this note. Never add a tag just because it is
in the list. When none of them fit, make a new tag
rather than forcing a poor match.";

pub fn user_message(body: &str, categories: &[String], existing_tags: &[String]) -> String {
    format!(
        "EXISTING CATEGORIES: {}\nEXISTING TAGS: {}\n\nNOTE:\n{}",
        categories.join(", "),
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

pub fn build(body: &str, categories: &[String], existing_tags: &[String]) -> String {
    chat(SYSTEM, &user_message(body, categories, existing_tags))
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
        assert!(SYSTEM.contains("3 to 8 words"));
        assert!(SYSTEM.contains("max 20 words"));
        assert!(SYSTEM.contains("1 to 5 lowercase tags"));
        // The two things that keep the vocabulary useful rather than merely
        // small: tag the subject matter, and never force-fit an existing tag.
        assert!(SYSTEM.contains("A tag is a shelf"));
        assert!(SYSTEM.contains("- category:"));
        assert!(SYSTEM.contains("what kind of note it is"));
        assert!(SYSTEM.contains("tag it by that name"));
        assert!(SYSTEM.contains("rather than forcing a poor match"));
        assert!(SYSTEM.contains("same language as the note"));
    }

    #[test]
    fn the_user_message_lists_categories_and_tags_then_the_note() {
        let message = user_message(
            "buy a hub",
            &["hardware".into()],
            &["argocd".into(), "homelab".into()],
        );
        assert_eq!(
            message,
            "EXISTING CATEGORIES: hardware\nEXISTING TAGS: argocd, homelab\n\nNOTE:\nbuy a hub"
        );
    }

    #[test]
    fn offers_at_most_the_top_tags() {
        let many: Vec<String> = (0..300).map(|i| format!("tag{i}")).collect();
        let message = user_message("note", &[], &many);
        let listed = message
            .lines()
            .nth(1)
            .unwrap()
            .trim_start_matches("EXISTING TAGS: ")
            .split(", ")
            .count();
        assert_eq!(listed, EXISTING_TAG_LIMIT);
    }

    #[test]
    fn an_empty_vocabulary_still_produces_a_usable_message() {
        let message = user_message("buy a hub", &[], &[]);
        assert!(message.starts_with("EXISTING CATEGORIES: \nEXISTING TAGS: \n\nNOTE:"));
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
