/**
 * The onboarding step to pick up on, kept while a run is under way so the
 * restart a new notes folder takes lands back in it. Storage can be missing,
 * in which case a run just starts from the top.
 */
const KEY = 'onboarding-step';

/** Null when no run is under way. */
export function resumeStep(): number | null {
	try {
		const saved = localStorage.getItem(KEY);
		const step = Number(saved);
		return saved !== null && Number.isInteger(step) && step >= 0 ? step : null;
	} catch {
		return null;
	}
}

export function setResumeStep(step: number) {
	try {
		localStorage.setItem(KEY, String(step));
	} catch {
		// Only a restart part way would miss it.
	}
}

export function clearResumeStep() {
	try {
		localStorage.removeItem(KEY);
	} catch {
		// Nothing to clear.
	}
}
