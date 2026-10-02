<script lang="ts">
	import { onMount } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import type { Note } from '#lib/plugins/api.js';
	import { m } from '#lib/paraglide/messages.js';
	import type { BasicsPlugin } from '../plugin.svelte';
	import { tasks, toggleTask } from './tasks';

	/** Every open task of the space (SPEC 3.8), on the plugin's page. */
	let { plugin, oncount }: { plugin: BasicsPlugin; oncount: (count: number) => void } = $props();

	const app = $derived(plugin.app);

	let notes = $state<Note[]>([]);
	let loaded = $state(false);
	let error = $state<string | null>(null);
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
			notes = await app.notes.containing(['[ ]', '[x]', '[X]']);
			error = null;
		} catch (e) {
			error = String(e);
		} finally {
			loaded = true;
		}
	}

	onMount(() => {
		void load();
		return app.on('notes-changed', () => void load());
	});

	const groups = $derived.by(() => {
		const found = notes
			.map((note) => {
				const body = unsaved.get(note.id) ?? note.body;
				const open = tasks(app.markdown, body).filter(
					(t) => !t.done || clicked.has(`${note.id}:${t.at}`)
				);
				return { note, body, open };
			})
			.filter((group) => group.open.length > 0);
		return plugin.settings.tasks.order === 'oldest' ? found.reverse() : found;
	});
	const count = $derived(
		groups.reduce((sum, group) => sum + group.open.filter((t) => !t.done).length, 0)
	);

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
				// Only ticks changed, so the note keeps its subject and category.
				await app.notes.setBody(note, next);
			} catch (e) {
				error = String(e);
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
	<p class="mb-4 text-sm text-red-400">{error}</p>
{/if}

{#if loaded && groups.length === 0}
	<p class="text-base text-neutral-600">{m.tasks_none()}</p>
{:else}
	<!-- On the day's own timeline: when on the left, the tasks on the right. -->
	<ul>
		{#each groups as { note, body, open } (note.id)}
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
{/if}

{#snippet pageIcon()}
	<FileTextIcon class="size-3" />
{/snippet}
