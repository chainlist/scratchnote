<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { saveNote } from '#lib/api.js';
	import NewNote from '#lib/components/NewNote.svelte';
	import NoteList from '#lib/components/NoteList.svelte';
	import { noteTitle } from '#lib/markdown.js';
	import View from '#lib/components/View.svelte';
	import { dayHeading, dayTitle, shortDay } from '#lib/components/ViewHeader.svelte';
	import { prettyHotkey } from '#lib/components/settings/HotkeyInput.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import type { Day } from './+page';

	let { data } = $props();

	const shell = getShell();
	const mac = navigator.userAgent.includes('Mac');

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
			return e instanceof Error ? e.message : String(e);
		}
	}

	/** The day's new note: the one under its notes, or the empty day's. */
	let newNote = $state<NewNote>();

	function onWindowKeydown(event: KeyboardEvent) {
		// Text keeps its keys, Alt+arrows move by word on macOS, and a dialog
		// or a menu keeps them from the day behind it.
		const target = event.target as HTMLElement;
		if (event.defaultPrevented || target.isContentEditable) return;
		if (target.closest('input, textarea, select, [role="dialog"], [role="menu"], [role="listbox"]'))
			return;
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
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- In a wider view the day has the one before it on its left, and in a wide
     one the one after it on its right too, which centres it. -->
<div
	class={[
		columns > 1 && 'grid justify-center gap-x-8',
		columns === 2 && 'grid-cols-[repeat(2,minmax(0,48rem))]',
		columns === 3 && 'grid-cols-[repeat(3,minmax(0,48rem))]'
	]}
>
	{#if columns > 1}{@render besideDay(data.previous)}{/if}
	<View key={data.date}>
		{#snippet heading(compact: boolean)}
			<!-- Side by side ahead of the title, so they stay put while its width
			     changes from one day to the next. -->
			<Button
				variant="ghost"
				size="icon-sm"
				href={data.previous && dayHref(data.previous.date)}
				disabled={!data.previous}
				aria-label={m.calendar_previous_day()}
				aria-keyshortcuts="Alt+ArrowLeft"
				title="{m.calendar_previous_day()} ({mac ? '⌥←' : 'Alt+←'})"
				class="text-muted-foreground hover:text-foreground"
			>
				<ChevronLeftIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				href={data.next && dayHref(data.next.date)}
				disabled={!data.next}
				aria-label={m.calendar_next_day()}
				aria-keyshortcuts="Alt+ArrowRight"
				title="{m.calendar_next_day()} ({mac ? '⌥→' : 'Alt+→'})"
				class="text-muted-foreground hover:text-foreground"
			>
				<ChevronRightIcon />
			</Button>
			<!-- The date opens the calendar, on the day's month; the calendar after
			     it says it can be clicked. -->
			<svelte:element
				this={compact ? 'span' : 'h1'}
				class={compact ? 'text-sm font-semibold whitespace-nowrap' : 'text-2xl font-medium'}
			>
				<a
					href={resolve('calendar/')}
					title={m.calendar_pick()}
					class="group/date -mx-1.5 inline-flex items-center gap-1.5 rounded-md px-1.5 transition-colors hover:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none dark:hover:bg-muted/50"
				>
					<span>
						{#if compact}{dayHeading(data.date, true)}{:else}{@render dayName(data.date)}{/if}
					</span>
					<CalendarIcon
						class={[
							'shrink-0 text-muted-foreground transition-colors group-hover/date:text-foreground',
							compact ? 'size-3.5' : 'size-4.5'
						]}
					/>
				</a>
			</svelte:element>
		{/snippet}

		<!-- What earlier notes said about this day (SPEC 5.3). -->
		{#if data.about.length}
			<!-- On the day's own columns, the day written where a note's time is,
			     so it reads as part of the page rather than a box laid on it. -->
			<section class="mb-8">
				<h2 class="mb-1 flex items-center gap-1.5 pl-24 text-xs font-medium text-meta">
					<CalendarClockIcon class="size-3.5" />{m.day_from_earlier()}
				</h2>
				<ul>
					{#each data.about as note (note.id)}
						<li>
							<button
								type="button"
								onclick={() => void shell.openCited(note)}
								title={m.day_written_on({ date: dayHeading(note.date) })}
								class="group -mx-3 grid w-[calc(100%+1.5rem)] cursor-pointer grid-cols-[4.5rem_1fr] items-baseline gap-x-6 rounded-lg px-3 py-1.5 text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
							>
								<!-- The day it was written, which brightens with a tick of the
								     accent, as a note's time does. -->
								<span
									class="relative text-right text-xs text-meta tabular-nums transition-colors group-hover:text-neutral-300 group-focus-visible:text-neutral-300"
								>
									{shortDay(note.date)}
									<span
										aria-hidden="true"
										class="absolute top-1/2 -right-3 h-3 w-0.5 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
									></span>
								</span>
								<span class="truncate text-sm text-neutral-300">{noteTitle(note)}</span>
							</button>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

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
</div>

<!-- A day's name as its column heads it: the weekday, then the date, quieter. -->
{#snippet dayName(date: string)}
	{@const title = dayTitle(date)}
	{title.weekday} <span class="font-normal whitespace-nowrap text-meta">{title.date}</span>
{/snippet}

<!-- A neighbour's column. Its date opens it; it has no new note of its own.
     Left empty before the first day, so the day keeps its place. -->
{#snippet besideDay(day: Day | undefined)}
	<section>
		{#if day}
			<a
				href={dayHref(day.date)}
				class="mt-6 mb-8 block max-w-full truncate text-lg leading-8 font-medium text-muted-foreground transition-colors hover:text-foreground"
			>
				{@render dayName(day.date)}
			</a>
			{#key day.date}
				<div class="page-in">
					<NoteList notes={day.notes} empty={m.page_empty_other_day()} {...shell.cardActions} />
				</div>
			{/key}
		{/if}
	</section>
{/snippet}
