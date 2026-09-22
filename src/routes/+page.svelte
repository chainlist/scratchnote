<script lang="ts">
	import { onMount } from 'svelte';
	import {
		deleteNote,
		getDay,
		listDays,
		onNoteUpdated,
		today,
		type DaySummary,
		type Note
	} from '$lib/api';
	import DayList from '$lib/components/DayList.svelte';
	import NoteCard from '$lib/components/NoteCard.svelte';

	let days = $state<DaySummary[]>([]);
	let notes = $state<Note[]>([]);
	let selected = $state('');
	let error = $state<string | null>(null);

	async function refresh() {
		try {
			[days, notes] = await Promise.all([listDays(), getDay(selected)]);
			error = null;
		} catch (e) {
			error = String(e);
		}
	}

	async function select(date: string) {
		selected = date;
		await refresh();
	}

	async function remove(note: Note) {
		try {
			await deleteNote(note.date, note.id);
			await refresh();
		} catch (e) {
			error = String(e);
		}
	}

	onMount(() => {
		let unlisten: Promise<() => void> | undefined;
		void (async () => {
			selected = await today();
			await refresh();
			// A note saved from the capture window lands in another webview.
			unlisten = onNoteUpdated(() => void refresh());
		})();
		return () => void unlisten?.then((off) => off());
	});

	const heading = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(undefined, {
					weekday: 'long',
					day: 'numeric',
					month: 'long',
					year: 'numeric'
				})
			: ''
	);
</script>

<div class="flex h-screen bg-neutral-950 text-neutral-100">
	<aside class="w-56 shrink-0 overflow-y-auto border-r border-neutral-800 p-3">
		<h1 class="mb-4 px-2 text-sm font-semibold">Scratchnote</h1>
		<DayList {days} {selected} onselect={select} />
	</aside>

	<main class="flex-1 overflow-y-auto p-6">
		<h2 class="mb-4 text-lg font-semibold">{heading}</h2>

		{#if error}
			<p class="rounded border border-red-900 bg-red-950 p-3 text-sm text-red-300">{error}</p>
		{/if}

		{#if notes.length === 0}
			<p class="text-sm text-neutral-600">
				Nothing captured. Press Ctrl+Shift+Space to write a note.
			</p>
		{:else}
			<ul class="flex flex-col gap-3">
				{#each notes as note (note.id)}
					<li><NoteCard {note} ondelete={remove} /></li>
				{/each}
			</ul>
		{/if}
	</main>
</div>
