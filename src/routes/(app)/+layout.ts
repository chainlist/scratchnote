import { redirect } from '@sveltejs/kit';
import { resolve } from '$app/paths';
import { embeddingModelInfo, getSettings, listDays, listSpaces, today } from '#lib/api.js';
import { resumeStep } from '#lib/onboarding.js';
import type { LayoutLoad } from './$types';

/** The onboarding check is made once, when the window first opens. */
let checked = false;

export const load: LayoutLoad = async ({ depends }) => {
	if (!checked) {
		// A fresh install goes through the onboarding first. Someone who
		// already has the embedding model is left alone. A run under way, as
		// after the restart a new folder takes, picks up again.
		const [settings, info] = await Promise.all([getSettings(), embeddingModelInfo()]);

		const fresh = !settings.onboarded && !info.installed && info.downloading === null;

		if (fresh || resumeStep() !== null) redirect(307, resolve('onboarding/'));

		checked = true;
	}

	// What every view uses: the days to step through, the spaces, and today.
	depends('app:notes');
	const [days, spaces, todayDate] = await Promise.all([listDays(), listSpaces(), today()]);
	return { days, spaces, today: todayDate };
};
