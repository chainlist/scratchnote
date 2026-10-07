/** How many days before today a day is: 0 for today, less for one ahead. */
export function daysAgo(date: string) {
	const now = new Date();
	const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
	// Rounded, as a day across a change of clocks is an hour off 24.
	return Math.round((today.getTime() - new Date(`${date}T00:00:00`).getTime()) / 86_400_000);
}

export type DayPart = 'morning' | 'afternoon' | 'evening' | 'night';

/** Minutes since midnight of a time such as `14:05`, or `9:05` as a hand
 *  edit of a day's file may leave it; null for any other text. */
export function clockMinutes(time: string): number | null {
	const clock = /^(\d{1,2}):(\d{2})(?!\d)/.exec(time);
	if (!clock) return null;
	const [hour, minute] = [Number(clock[1]), Number(clock[2])];
	return hour < 24 && minute < 60 ? hour * 60 + minute : null;
}

/** The part of the day a time such as `14:05` falls in: morning from 05:00,
 *  afternoon from 12:00, evening from 17:00, night from 22:00 until morning.
 *  None for a time that cannot be read. */
export function dayPart(time: string): DayPart | undefined {
	const minutes = clockMinutes(time);
	if (minutes === null) return undefined;
	const hour = Math.floor(minutes / 60);
	if (hour >= 5 && hour < 12) return 'morning';
	if (hour >= 12 && hour < 17) return 'afternoon';
	if (hour >= 17 && hour < 22) return 'evening';
	return 'night';
}

/** Minutes between two times of one day, such as `09:20` and `14:05`;
 *  none when either cannot be read. */
export function minutesBetween(a: string, b: string) {
	const [from, to] = [clockMinutes(a), clockMinutes(b)];
	return from === null || to === null ? 0 : Math.abs(to - from);
}

/**
 * The room a timeline leaves above an item for the time since the one
 * before it, in rem: none within twenty minutes, so a busy hour stays
 * close, then a rem for each hour after, so an afternoon reads as a
 * silence, and capped near a short note's height, so a long silence never
 * pushes the next note out of sight.
 */
export function gapRoom(minutes: number) {
	return Math.min(4, Math.max(0, (minutes - 20) / 60));
}

/**
 * Where each item of one day's timeline starts a part of the day, and the
 * minutes since the item before it: what `TimelineItem` spaces them by.
 * Items spanning days get neither, as their days stand between them.
 */
export function daySpacing(
	items: { time: string; date: string }[]
): { part?: DayPart; gapMinutes: number }[] {
	// The part named last, so a time that cannot be read names none and
	// does not make the next one name its part again.
	let named: DayPart | undefined;
	return items.map((item, i) => {
		const before = items[i - 1];
		if (before && before.date !== item.date) return { gapMinutes: 0 };
		const part = dayPart(item.time);
		const opens = part !== undefined && part !== named;
		if (part) named = part;
		return {
			part: opens ? part : undefined,
			gapMinutes: before ? minutesBetween(before.time, item.time) : 0
		};
	});
}
