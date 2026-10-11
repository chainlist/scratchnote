import { getDay, notesAbout, today as readToday, type Note } from '#lib/api.js';
import { plusDays } from '#lib/helpers/dates.js';
import type { PageLoad } from './$types';

// Any day, so none is known ahead to prerender; the fallback page serves it.
export const prerender = false;

export interface Day {
	date: string;
	notes: Note[];
}

/**
 * The day, and its neighbours: the days the arrows step to, which the view
 * shows beside it when they fit. Also the notes that look forward to it, and
 * to the days with no notes before it.
 */
export const load: PageLoad = async ({ params, parent, depends }) => {
	depends('app:notes');
	// Today as it is now: the window can stay open past midnight, and the
	// layout reads it again only once notes change.
	const [{ days }, today] = await Promise.all([parent(), readToday()]);
	const date = params.date;
	// The days the arrows step through: those with notes, and today. Dates
	// sort as strings.
	const written = days.map((day) => day.date).sort();
	const stops = [...new Set([...written, today])].sort();
	const day = async (date: string | undefined): Promise<Day | undefined> =>
		date === undefined ? undefined : { date, notes: await getDay(date) };
	// What looked forward to the days with no notes since the last day that
	// had some comes to the next day written, or to today.
	const lastWritten = written.findLast((other) => other < date);
	const carries =
		date <= today && (date === today || written.includes(date)) && lastWritten !== undefined;
	const from = carries ? plusDays(lastWritten, 1) : date;
	const [notes, ahead, previous, next] = await Promise.all([
		getDay(date),
		notesAbout(from, date),
		day(stops.findLast((other) => other < date)),
		day(stops.find((other) => other > date))
	]);
	return {
		date,
		today,
		notes,
		about: ahead.filter((note) => note.on === date),
		skipped: ahead.filter((note) => note.on !== date),
		previous,
		next
	};
};
