<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { saveNote } from '#lib/api.js';
	import NewNote from './NewNote.svelte';
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/View.svelte';
	import { dayHeading } from '#lib/dates.js';
	import { prettyHotkey } from '#lib/hotkeys.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { typesText } from '#lib/dom.js';
	import { errorText } from '#lib/errors.js';
	import type { Day } from './+page';
	import DayName from './DayName.svelte';
	import DayTitleRow from './DayTitleRow.svelte';
	import EarlierNotes from './EarlierNotes.svelte';
	import { slideDays, type Sliding } from './day-swipe.js';

	let { data } = $props();

	const shell = getShell();

	// The day shown is the one the other views go back to.
	$effect(() => {
		shell.day = data.date;
	});

	/** How many days fit side by side, each at least 36rem wide: the day
	 *  alone, then with the day before it from 77rem, then between both
	 *  neighbours from 115rem. A day with none after it, as today, keeps
	 *  to two rather than leave a column empty. */
	const columns = $derived.by(() => {
		const rems = shell.width / shell.textSize;
		const fit = rems >= 115 ? 3 : rems >= 77 ? 2 : 1;
		return data.next ? fit : Math.min(fit, 2);
	});

	/** The day whose empty-day editor is open, which takes the place of its message. */
	let writingOn = $state<string | null>(null);
	/** Where a new note's draft is kept while the day is left. */
	const draftKey = $derived(`${data.spaces.active}/${data.date}`);

	const dayHref = (date: string) => resolve(`day/${date}/`);

	/** Add a note to the day shown. Resolves to the error as it came, which
	 *  the editor shows under itself, or null once saved. */
	async function addNote(body: string): Promise<string | null> {
		try {
			await saveNote(body, data.date);
			await shell.refresh();
			return null;
		} catch (e) {
			return errorText(e);
		}
	}

	/** The day's new note: the one under its notes, or the empty day's. */
	let newNote = $state<NewNote>();

	function onWindowKeydown(event: KeyboardEvent) {
		// Text keeps its keys, Alt+arrows move by word on macOS, and a dialog
		// or a menu keeps them from the day behind it.
		const target = event.target as HTMLElement;
		if (event.defaultPrevented || typesText(target)) return;
		if (target.closest('select, [role="dialog"], [role="menu"], [role="listbox"]')) return;
		// N starts a note, as the button under the day's notes does.
		if (event.key.toLowerCase() === 'n' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			if (event.repeat) return;
			event.preventDefault();
			void newNote?.start();
			return;
		}
		if (!event.altKey || (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')) return;
		// Also stops the webview going back in its history.
		event.preventDefault();
		const day = event.key === 'ArrowLeft' ? data.previous : data.next;
		if (day) void goto(dayHref(day.date));
	}

	/** The view's column and what slides with it. */
	let track = $state<HTMLElement | null>(null);
	/** The day the finger brings in, beside the view, while it moves. */
	let sliding = $state<Sliding | null>(null);

	// When the day is alone in the view, a swipe slides in the day before or
	// after, as the arrows go to: it follows the finger, and lands as the view
	// opens on it. Where there is no day to go to, the view holds back.
	$effect(() => {
		const main = shell.main;
		const column = track;
		if (!main || !column) return;
		return slideDays(main, column, {
			alone: () => columns === 1,
			beside: (before) => (before ? data.previous : data.next),
			get sliding() {
				return sliding;
			},
			set sliding(next) {
				sliding = next;
			},
			open: async (day) => {
				shell.slid = true;
				try {
					await goto(dayHref(day.date));
				} finally {
					shell.slid = false;
				}
			}
		});
	});
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- In a wider view the day has the one before it on its left, and in a wide
     one the one after it on its right too, which centres it. -->
<div
	bind:this={track}
	class={[
		// Holds its title's top margin, as the day sliding in beside it does.
		'relative',
		columns === 1 && 'flow-root',
		columns > 1 && 'grid justify-center gap-x-8',
		columns === 2 && 'grid-cols-[repeat(2,minmax(0,48rem))]',
		columns === 3 && 'grid-cols-[repeat(3,minmax(0,48rem))]'
	]}
>
	{#if columns > 1}{@render besideDay(data.previous)}{/if}
	<View key={data.date} name={dayHeading(data.date)}>
		{#snippet heading(compact: boolean)}
			<DayTitleRow
				date={data.date}
				previous={data.previous?.date}
				next={data.next?.date}
				{compact}
			/>
		{/snippet}

		{#if data.about.length}<EarlierNotes notes={data.about} />{/if}

		{#if data.notes.length === 0}
			<div class="flex flex-col items-center gap-4 py-16 text-center">
				{#if writingOn !== data.date}
					<!-- Today names the capture hotkey once it is read, and only when one is set. -->
					<p class="text-base text-meta">
						{#if data.date > data.today}
							{m.page_empty_future_day()}
						{:else if data.date < data.today || shell.captureHotkey === ''}
							{m.page_empty_other_day()}
						{:else if shell.captureHotkey}
							{m.page_empty_day({ hotkey: prettyHotkey(shell.captureHotkey) })}
						{/if}
					</p>
				{/if}
				<NewNote
					bind:this={newNote}
					{draftKey}
					onsave={addNote}
					onpage={shell.newPage}
					onerror={shell.showError}
					centered
					bind:writing={
						() => writingOn === data.date, (open) => (writingOn = open ? data.date : null)
					}
				/>
			</div>
		{:else}
			<NoteList notes={data.notes} empty="" blinking={shell.blinking} {...shell.cardActions} />
			<NewNote
				bind:this={newNote}
				{draftKey}
				onsave={addNote}
				onpage={shell.newPage}
				onerror={shell.showError}
			/>
		{/if}
	</View>
	{#if columns === 3}{@render besideDay(data.next)}{/if}
	{#if sliding}
		<!-- The day the finger brings in, drawn as the view will draw it, at the
		     top of what shows. Only a picture of it until it lands. -->
		<div
			inert
			aria-hidden="true"
			class="absolute inset-x-0 mx-auto w-full max-w-3xl"
			style:top="{sliding.top}px"
			style:transform="translateX({sliding.offset}px)"
		>
			<div class="mt-6 mb-8 flex items-center gap-2">
				<!-- Its own neighbours are not loaded: its arrows show both, but
				     the one back to this day. -->
				<DayTitleRow
					date={sliding.day.date}
					previous={sliding.day.date}
					next={sliding.day.date}
					compact={false}
				/>
			</div>
			{#if sliding.day.notes.length}
				<NoteList notes={sliding.day.notes} empty="" {...shell.cardActions} />
			{:else}
				<p class="py-16 text-center text-base text-meta">{m.page_empty_other_day()}</p>
			{/if}
		</div>
	{/if}
</div>

<!-- A neighbour's column. Its date opens it; it has no new note of its own.
     Left empty before the first day, so the day keeps its place. -->
{#snippet besideDay(day: Day | undefined)}
	<section>
		{#if day}
			<a
				href={dayHref(day.date)}
				class="mt-6 mb-8 block max-w-full truncate text-lg leading-8 font-medium text-muted-foreground transition-colors hover:text-foreground"
			>
				<DayName date={day.date} />
			</a>
			{#key day.date}
				<div class="page-in">
					<NoteList notes={day.notes} empty={m.page_empty_other_day()} {...shell.cardActions} />
				</div>
			{/key}
		{/if}
	</section>
{/snippet}
