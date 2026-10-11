import { notesAbout, today as readToday } from '#lib/api.js';
import { plusDays } from '#lib/helpers/dates.js';
import type { PageLoad } from './$types';

/** How many days ahead the calendar lists the notes that look forward to them (SPEC 5.3). */
const COMING_DAYS = 7;

/**
 * The notes that look forward to the days ahead: the month marks each of
 * those days, and the week to come is listed beside or under it.
 */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	// Today as it is now, as the day page reads it.
	const today = await readToday();
	const ahead = await notesAbout(plusDays(today, 1), '9999-12-31');
	const week = plusDays(today, COMING_DAYS);
	return {
		today,
		ahead,
		coming: ahead.filter((note) => note.on !== undefined && note.on <= week)
	};
};
