<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { saveNote } from '$lib/api';
	import NewNote from '$lib/components/NewNote.svelte';
	import NoteList from '$lib/components/NoteList.svelte';
	import View from '$lib/components/View.svelte';
	import { dayHeading } from '$lib/components/ViewHeader.svelte';
	import { Button } from '$lib/components/ui/button';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';
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

	const dayHref = (date: string) => resolve(`/day/${date}/`);

	/** Add a note to the day shown. */
	async function addNote(body: string): Promise<boolean> {
		try {
			await saveNote(body, data.date);
			shell.error = null;
			await shell.refresh();
			return true;
		} catch (e) {
			shell.error = String(e);
			return false;
		}
	}

	function onWindowKeydown(event: KeyboardEvent) {
		if (!event.altKey || (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')) return;
		// Text keeps its Alt+arrows, which move by word on macOS, and a
		// dialog keeps them from the day behind it.
		const target = event.target as HTMLElement;
		if (target.isContentEditable || target.closest('input, textarea, [role="dialog"]')) return;
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
			<svelte:element
				this={compact ? 'span' : 'h1'}
				class={compact
					? 'text-sm font-semibold whitespace-nowrap'
					: 'text-2xl font-semibold tracking-tight'}
			>
				{dayHeading(data.date, compact)}
			</svelte:element>
		{/snippet}

		{#if data.notes.length === 0}
			<div class="flex flex-col items-center gap-4 py-16 text-center">
				{#if writingOn !== data.date}
					<p class="text-base text-neutral-600">
						{data.date === data.today
							? m.page_empty_day({ hotkey: 'Ctrl+Shift+Space' })
							: m.page_empty_other_day()}
					</p>
				{/if}
				<NewNote
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
			<NewNote onsave={addNote} onpage={shell.newPage} onerror={shell.showError} />
		{/if}
	</View>
	{#if columns === 3}{@render besideDay(data.next)}{/if}
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
				{dayHeading(day.date)}
			</a>
			{#key day.date}
				<div class="page-in">
					<NoteList notes={day.notes} empty={m.page_empty_other_day()} {...shell.cardActions} />
				</div>
			{/key}
		{/if}
	</section>
{/snippet}
