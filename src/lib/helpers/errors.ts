/** What went wrong, to show: an Error's message, or what was thrown, as text. */
export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
