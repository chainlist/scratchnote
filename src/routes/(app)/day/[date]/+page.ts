import { getDay, type Note } from '$lib/api';
import type { PageLoad } from './$types';

// Any day, so none is known ahead to prerender; the fallback page serves it.
export const prerender = false;

export interface Day {
	date: string;
	notes: Note[];
}

/**
 * The day, and its neighbours: the days the arrows step to, which the view
 * shows beside it when they fit.
 */
export const load: PageLoad = async ({ params, parent, depends }) => {
	depends('app:notes');
	const { days, today } = await parent();
	// The days the arrows step through: those with notes, and today. Dates
	// sort as strings.
	const stops = [...new Set([...days.map((day) => day.date), today])].sort();
	const day = async (date: string | undefined): Promise<Day | undefined> =>
		date === undefined ? undefined : { date, notes: await getDay(date) };
	const [notes, previous, next] = await Promise.all([
		getDay(params.date),
		day(stops.findLast((date) => date < params.date)),
		day(stops.find((date) => date > params.date))
	]);
	return { date: params.date, notes, previous, next };
};
