import { redirect } from '@sveltejs/kit';
import { resolve } from '$app/paths';
import {
	categoryNames,
	getSettings,
	listCategories,
	listDays,
	listSpaces,
	modelInfo,
	modelStatus,
	today
} from '#lib/api.js';
import { resumeStep } from '#lib/onboarding.js';
import type { LayoutLoad } from './$types';

/** The onboarding check is made once, when the window first opens. */
let checked = false;

export const load: LayoutLoad = async ({ depends }) => {
	if (!checked) {
		// A fresh install goes through the onboarding first. Someone who
		// already has a model, or turned it off, is left alone. A run under
		// way, as after the restart a new folder takes, picks up again.
		const [settings, info] = await Promise.all([getSettings(), modelInfo()]);

		const fresh =
			!settings.onboarded &&
			settings.modelEnabled &&
			!info.activePath &&
			!info.light &&
			!info.default;

		if (fresh || resumeStep() !== null) redirect(307, resolve('onboarding/'));

		checked = true;
	}

	// What every view uses: the days to step through, the categories to
	// filter and file by, the spaces, and today. The model's state too, so
	// the first paint does not offer to install a model that is there.
	depends('app:notes');
	const [days, categories, categoryList, spaces, todayDate, model] = await Promise.all([
		listDays(),
		listCategories(),
		categoryNames(),
		listSpaces(),
		today(),
		modelStatus()
	]);
	return { days, categories, categoryList, spaces, today: todayDate, model };
};
