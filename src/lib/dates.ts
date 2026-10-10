import { getLocale } from '#lib/paraglide/runtime.js';

/**
 * Days and times as the notes write them (`2026-10-06`, `14:05`), and as
 * the app shows them in the interface language. Core plugins format their
 * own, with `app.locale`, as plugins only have the plugin API.
 */

/** A day such as `2026-10-06` as its local midnight. */
export const localDay = (date: string) => new Date(`${date}T00:00:00`);

/** A local date as the notes write their days: `2026-10-06`. */
export function isoDay(day: Date) {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${day.getFullYear()}-${pad(day.getMonth() + 1)}-${pad(day.getDate())}`;
}

/** How many days before today a day is: 0 for today, less for one ahead. */
export function daysAgo(date: string) {
	const now = new Date();
	const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
	// Rounded, as a day across a change of clocks is an hour off 24.
	return Math.round((today.getTime() - localDay(date).getTime()) / 86_400_000);
}

const DAY_MS = 86_400_000;

/** A day as a whole number of days since 1970, to measure spans of days in. */
export const dayNumber = (date: string) => Math.round(Date.parse(`${date}T00:00:00Z`) / DAY_MS);

/** The day `dayNumber` counted, as the notes write it. */
export const dayOfNumber = (day: number) => new Date(day * DAY_MS).toISOString().slice(0, 10);

/** The formats made, by language and options: making one takes far longer
 *  than using it, and a long list formats a date on every row. */
const formats = new Map<string, Intl.DateTimeFormat>();

/** A format for dates in the interface language, made once for each options. */
export function dateFormat(options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
	const key = `${getLocale()} ${JSON.stringify(options)}`;
	let format = formats.get(key);
	if (!format) formats.set(key, (format = new Intl.DateTimeFormat(getLocale(), options)));
	return format;
}

/** A day formatted, or `Invalid Date` for text that is no day, as
 *  `toLocaleDateString` has it, rather than an error. */
function formatDay(day: Date, options: Intl.DateTimeFormatOptions) {
	return Number.isNaN(day.getTime()) ? String(day) : dateFormat(options).format(day);
}

/** Its first letter capitalised, as a title's first word is. */
const capitalised = (text: string) => text.charAt(0).toLocaleUpperCase(getLocale()) + text.slice(1);

const thisYear = (day: Date) => day.getFullYear() === new Date().getFullYear();

const LONG: Intl.DateTimeFormatOptions = {
	weekday: 'long',
	day: 'numeric',
	month: 'long',
	year: 'numeric'
};
const SHORT: Intl.DateTimeFormatOptions = {
	weekday: 'short',
	day: 'numeric',
	month: 'short',
	year: 'numeric'
};

/**
 * A day as its title reads, or shorter, as the top bar has it. A view
 * being left reads its title once more with the next route's data, which
 * may hold no date: that reads `Invalid Date`, with no error.
 */
export const dayHeading = (date: string, short = false) =>
	formatDay(localDay(date), short ? SHORT : LONG);

/** A day's title in two parts, as a journal page heads its day: the
 *  weekday, capitalised as a title's first word is, then the date. */
export function dayTitle(date: string): { weekday: string; date: string } {
	const day = localDay(date);
	if (Number.isNaN(day.getTime())) return { weekday: String(day), date: '' };
	const parts = dateFormat(LONG).formatToParts(day);
	const weekday = parts.find((part) => part.type === 'weekday')?.value ?? '';
	const rest = parts
		.filter((part) => part.type !== 'weekday')
		.map((part) => part.value)
		.join('')
		.replace(/^[\s,]+|[\s,]+$/g, '');
	return { weekday: capitalised(weekday), date: rest };
}

/** A day in a few characters, as a line names it: the year only when it is not this one. */
export function shortDay(date: string) {
	const day = localDay(date);
	const year = thisYear(day) ? {} : { year: 'numeric' as const };
	return dateFormat({ day: 'numeric', month: 'short', ...year }).format(day);
}

/** A day ahead in a few characters, its weekday first: the year only when it is not this one. */
export function aheadLabel(date: string) {
	const day = localDay(date);
	const year = thisYear(day) ? {} : { year: 'numeric' as const };
	return formatDay(day, { weekday: 'short', day: 'numeric', month: 'short', ...year });
}

/** A month such as `2026-10` as its heading reads, its year left out in this one. */
export function monthHeading(month: string) {
	const day = localDay(`${month}-01`);
	const year = thisYear(day) ? {} : { year: 'numeric' as const };
	return capitalised(dateFormat({ month: 'long', ...year }).format(day));
}

/** A day's weekday, short: `Tue`. */
export const weekday = (date: string) => dateFormat({ weekday: 'short' }).format(localDay(date));

/** A day in full but for its weekday, as a release is dated. */
export const longDay = (date: string) =>
	formatDay(localDay(date), { day: 'numeric', month: 'long', year: 'numeric' });

export type DayPart = 'morning' | 'afternoon' | 'evening' | 'night';

/** Minutes since midnight of a time such as `14:05`, or `9:05` as a hand
 *  edit of a day's file may leave it; null for any other text. */
export function clockMinutes(time: string): number | null {
	const clock = /^(\d{1,2}):(\d{2})(?!\d)/.exec(time);
	if (!clock) return null;
	const [hour, minute] = [Number(clock[1]), Number(clock[2])];
	return hour < 24 && minute < 60 ? hour * 60 + minute : null;
}

/** Minutes since midnight as a clock reads them, `09:05`. */
export function clockTime(minutes: number) {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
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
