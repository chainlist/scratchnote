import { readJson, remove, writeJson } from '#lib/helpers/storage.js';

/**
 * The onboarding step to pick up on, kept while a run is under way so the
 * restart a new notes folder takes lands back in it. Storage can be missing,
 * in which case a run just starts from the top.
 */
const KEY = 'onboarding-step';

/** Null when no run is under way. */
export function resumeStep(): number | null {
	const step = readJson(KEY);
	return typeof step === 'number' && Number.isInteger(step) && step >= 0 ? step : null;
}

export const setResumeStep = (step: number) => writeJson(KEY, step);

export const clearResumeStep = () => remove(KEY);
