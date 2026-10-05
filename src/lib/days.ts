/** How many days before today a day is: 0 for today, less for one ahead. */
export function daysAgo(date: string) {
	const now = new Date();
	const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
	// Rounded, as a day across a change of clocks is an hour off 24.
	return Math.round((today.getTime() - new Date(`${date}T00:00:00`).getTime()) / 86_400_000);
}
