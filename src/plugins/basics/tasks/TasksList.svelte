<script lang="ts">
	import { onMount } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import InlineError from '#lib/components/common/InlineError.svelte';
	import TimelineItem from '#lib/components/note/TimelineItem.svelte';
	import ShowMore from '#lib/components/common/ShowMore.svelte';
	import type { Note } from '#lib/plugins/api.js';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/notes/query.js';
	import type { BasicsPlugin } from '../plugin.svelte';
	import { tasks, toggleTask, type Task } from './tasks';

	/** Every open task of the space (SPEC 3.8), on the plugin's page. */
	let { plugin, oncount }: { plugin: BasicsPlugin; oncount: (count: number) => void } = $props();

	const app = $derived(plugin.app);

	let notes = $state<Note[]>([]);
	let loaded = $state(false);
	let error = $state<{ what: string; detail: string } | null>(null);
	/** Bodies with boxes clicked but not saved yet, shown meanwhile. */
	const unsaved = new SvelteMap<string, string>();
	/** The tasks clicked here, by note and box. They stay in the list, ticked,
	 *  until the view closes, so a box ticked by mistake can be cleared again. */
	const clicked = new SvelteSet<string>();
	let saving = Promise.resolve();

	const isPage = (note: Note) => note.kind === 'page';

	/** The notes and pages with a box, newest first; the view keeps the open tasks. */
	async function load() {
		try {
			const fresh = await app.notes.containing(['[ ]', '[x]', '[X]']);
			// A note unchanged keeps its object, so its lines are not drawn again
			// when one box is saved.
			const before = new Map(notes.map((note) => [note.id, note]));
			notes = fresh.map((note) => {
				const old = before.get(note.id);
				return old &&
					old.body === note.body &&
					old.time === note.time &&
					old.date === note.date &&
					old.subject === note.subject
					? old
					: note;
			});
			error = null;
		} catch (e) {
			error = { what: m.error_load_tasks(), detail: String(e) };
		} finally {
			loaded = true;
		}
	}

	onMount(() => {
		void load();
		return app.on('notes-changed', () => void load());
	});

	/** A body's tasks, read once per text: a click re-reads only its own note. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a cache, nothing is drawn from it
	const read = new Map<string, Task[]>();
	function tasksOf(body: string) {
		let found = read.get(body);
		if (!found) read.set(body, (found = tasks(app.markdown, body)));
		return found;
	}

	type Group = { note: Note; body: string; open: Task[] };
	/** Each note's group as last listed: kept while nothing in it changed, so
	 *  a click draws again only the lines of its own note. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a cache the list reads, never draws from
	const listed = new Map<string, Group>();

	const groups = $derived.by(() => {
		const found = notes
			.map((note) => {
				const body = unsaved.get(note.id) ?? note.body;
				const open = tasksOf(body).filter((t) => !t.done || clicked.has(`${note.id}:${t.at}`));
				const last = listed.get(note.id);
				if (
					last?.note === note &&
					last.body === body &&
					last.open.length === open.length &&
					last.open.every((task, i) => task === open[i])
				)
					return last;
				const group = { note, body, open };
				listed.set(note.id, group);
				return group;
			})
			.filter((group) => group.open.length > 0);
		return plugin.settings.tasks.order === 'oldest' ? found.reverse() : found;
	});
	/** How many notes' tasks are drawn: a stretch at first, as many more on
	 *  asking, as the other long lists do, so a space with thousands of open
	 *  tasks opens at once. The count in the title is of all of them. */
	let drawn = $state(RESULTS_STEP);
	/** The open tasks of these notes. */
	const openIn = (shown: typeof groups) =>
		shown.reduce((sum, group) => sum + group.open.filter((t) => !t.done).length, 0);
	const count = $derived(openIn(groups));
	/** Those of the notes not drawn yet, for Show more. */
	const tasksLeft = $derived(openIn(groups.slice(drawn)));

	// The page's title counts them.
	$effect(() => oncount(count));

	/** A box clicked saves at once; clicks in a row save one after another,
	 *  each on the text the one before left. */
	function toggle(note: Note, at: number) {
		const next = toggleTask(unsaved.get(note.id) ?? note.body, at);
		unsaved.set(note.id, next);
		clicked.add(`${note.id}:${at}`);
		saving = saving.then(async () => {
			try {
				await app.notes.setBody(note, next);
			} catch (e) {
				error = { what: m.error_save_note(), detail: String(e) };
			}
			if (unsaved.get(note.id) === next) unsaved.delete(note.id);
		});
	}

	/** A task's line drawn as its card draws it, its box ticking this one. */
	const line =
		(text: string, ontick: () => void): Attachment<HTMLElement> =>
		(el) => {
			const drawn = app.markdown.render(el, text, {
				onchange: ontick,
				class: 'text-base leading-7 text-neutral-200'
			});
			return () => drawn.destroy();
		};
</script>

{#if error}
	<InlineError class="mb-4" message={error.what} detail={error.detail} />
{/if}

{#if loaded && groups.length === 0}
	<p class="text-base text-meta">{m.tasks_none()}</p>
{:else}
	<!-- On the day's own timeline: when on the left, the tasks on the right. -->
	<ul>
		{#each groups.slice(0, drawn) as { note, body, open } (note.id)}
			<li>
				<TimelineItem
					time={note.time}
					date={note.date}
					ontime={() => app.workspace.openNote(note)}
					timeTitle={isPage(note) ? m.pages_open() : m.tasks_show_in_day()}
					icon={isPage(note) ? pageIcon : undefined}
				>
					<div class="min-w-0">
						{#if isPage(note)}
							<button
								type="button"
								onclick={() => app.workspace.openNote(note)}
								class="max-w-full cursor-pointer truncate text-sm leading-7 font-medium text-neutral-400 transition-colors hover:text-neutral-200"
							>
								{note.subject ?? m.pages_untitled()}
							</button>
						{/if}
						{#each open as task (task.at)}
							<div
								{@attach line(body.slice(task.from, task.to), () => toggle(note, task.at))}
							></div>
						{/each}
					</div>
				</TimelineItem>
			</li>
		{/each}
	</ul>
	{#if drawn < groups.length}
		<!-- Tasks, as the title counts them, not notes. -->
		<ShowMore class="mt-8" count={tasksLeft} onclick={() => (drawn += RESULTS_STEP)} />
	{/if}
{/if}

{#snippet pageIcon()}
	<FileTextIcon class="size-3" />
{/snippet}
