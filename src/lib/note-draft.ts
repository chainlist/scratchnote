/**
 * The new notes being written under a day, not saved yet, by space and day,
 * and whether their editor was open. They outlive the day view, as the
 * capture window's draft outlives an Esc and a new page's its view, so going
 * to another day or view and back finds the note as it was left.
 */
export const noteDrafts = new Map<string, { body: string; writing: boolean }>();

/**
 * The changes made to notes and left unsaved, by note: an edit closed with
 * Esc keeps them, as a new note's editor keeps its draft, until they are
 * saved or discarded.
 */
export const editDrafts = new Map<string, string>();
