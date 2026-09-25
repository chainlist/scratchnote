//! The prompt from SPEC 5.4, wrapped in Qwen3's chat template.

/// Notes longer than this are truncated for enrichment only, head and tail
/// kept, so a long note still gets labelled without blowing the context.
/// Roughly the ~3000 tokens SPEC 5.1 allows, counted in characters.
const MAX_NOTE_CHARS: usize = 12_000;

/// How many of the existing tags to offer the model (SPEC 5.4).
pub const EXISTING_TAG_LIMIT: usize = 150;

/// `language` is the English name of the one the user picked, such as
/// "French": the labels follow it whatever language the note is in.
pub fn system(language: &str) -> String {
    format!(
        "\
You label short personal notes. Return JSON only.
Write subject, summary, category and tags in {language}, whatever language the
note is in: a note in another language is labelled in {language}, translated.
Names of projects, products, tools, people and places stay as they are.
- subject: a title of 3 to 8 words naming what the note is about. Do not copy
  the note's first sentence. No trailing punctuation.
- summary: one sentence, max 20 words, stating the key point, decision or next
  action.
- category: one lowercase word or kebab-case naming the broad subject the note
  belongs to, such as its field, technology, hobby or medium, so it sits next
  to related notes that name other things. Not what kind of note it is.
  Pick it from the EXISTING CATEGORIES written in {language}, spelled exactly
  as listed. Only when none of them is the subject of the note, make a new one
  in {language}.
- tags: 1 to 5 lowercase tags in {language}, single words or kebab-case. A
  subject word from a note in another language is translated.
  A tag is a shelf the user browses later: keep one only if they would want to
  see other notes under it.
  First, when the note names a specific project, product, tool, game, film,
  show, person or place, tag it by that name. A project or product the note
  names is always a tag, even when the note is about one feature of it. Then
  add at most two main subjects it covers. Do not repeat the category.
  Never tag a phrase lifted from the note, a version number, a small detail, or
  what kind of note it is, such as issue, plan, update, task, idea, todo or
  question.
EXISTING TAGS only tells you how a subject is already spelled. Reuse one only
when it is a name or a {language} word, and means the same thing in this note.
Never add a tag just because it is in the list. When none of them fit, make a
new tag
rather than forcing a poor match."
    )
}

/// The language is named again after the note: said only in the system
/// message, the 4B labels an English note in English whatever it asks. The
/// categories and tags offered must be that language's too (`Space::vocabulary`),
/// or it reuses them whatever it is told.
pub fn user_message(
    body: &str,
    language: &str,
    categories: &[String],
    existing_tags: &[String],
) -> String {
    format!(
        "EXISTING CATEGORIES: {}\nEXISTING TAGS: {}\n\nNOTE:\n{}\n\nLABEL IN: {language}. The category and every tag are {language} words, except names.",
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

pub fn build(
    body: &str,
    language: &str,
    categories: &[String],
    existing_tags: &[String],
) -> String {
    chat(
        &system(language),
        &user_message(body, language, categories, existing_tags),
    )
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
        let system = system("French");
        assert!(system.starts_with("You label short personal notes. Return JSON only."));
        assert!(system.contains("3 to 8 words"));
        assert!(system.contains("max 20 words"));
        assert!(system.contains("1 to 5 lowercase tags"));
        // The two things that keep the vocabulary useful rather than merely
        // small: tag the subject matter, and never force-fit an existing tag.
        assert!(system.contains("A tag is a shelf"));
        assert!(system.contains("- category:"));
        assert!(system.contains("what kind of note it is"));
        assert!(system.contains("tag it by that name"));
        assert!(system.contains("rather than forcing a poor match"));
        assert!(system.contains("tags in French, whatever language the"));
    }

    #[test]
    fn the_user_message_lists_categories_and_tags_then_the_note_and_language() {
        let message = user_message(
            "buy a hub",
            "French",
            &["hardware".into()],
            &["argocd".into(), "homelab".into()],
        );
        assert_eq!(
            message,
            "EXISTING CATEGORIES: hardware\nEXISTING TAGS: argocd, homelab\n\nNOTE:\nbuy a hub\n\nLABEL IN: French. The category and every tag are French words, except names."
        );
    }

    #[test]
    fn offers_at_most_the_top_tags() {
        let many: Vec<String> = (0..300).map(|i| format!("tag{i}")).collect();
        let message = user_message("note", "English", &[], &many);
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
        let message = user_message("buy a hub", "English", &[], &[]);
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
