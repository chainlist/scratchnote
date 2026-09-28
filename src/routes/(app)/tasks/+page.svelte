<script lang="ts">
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { finishPage, isPage, updateNote, updatePage, type Note } from '$lib/api';
	import Markdown from '$lib/components/Markdown.svelte';
	import View from '$lib/components/View.svelte';
	import { tasks, toggleTask } from '$lib/markdown';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';

	let { data } = $props();

	const shell = getShell();

	/** Bodies with boxes clicked but not saved yet, shown meanwhile. */
	const unsaved = new SvelteMap<string, string>();
	/** The tasks clicked here, by note and box. They stay in the list, ticked,
	 *  until the view closes, so a box ticked by mistake can be cleared again. */
	const clicked = new SvelteSet<string>();
	let saving = Promise.resolve();

	const groups = $derived(
		data.notes
			.map((note) => {
				const body = unsaved.get(note.id) ?? note.body;
				const open = tasks(body).filter((t) => !t.done || clicked.has(`${note.id}:${t.at}`));
				return { note, body, open };
			})
			.filter((group) => group.open.length > 0)
	);
	const count = $derived(
		groups.reduce((sum, group) => sum + group.open.filter((t) => !t.done).length, 0)
	);

	/** A box clicked saves at once; clicks in a row save one after another,
	 *  each on the text the one before left. */
	function toggle(note: Note, at: number) {
		const next = toggleTask(unsaved.get(note.id) ?? note.body, at);
		unsaved.set(note.id, next);
		clicked.add(`${note.id}:${at}`);
		saving = saving.then(async () => {
			await save(note, next);
			if (unsaved.get(note.id) === next) unsaved.delete(note.id);
		});
	}

	async function save(note: Note, body: string) {
		try {
			if (isPage(note)) {
				await updatePage(note.id, body);
				// A save holds the page, as the page view's saves do. Only
				// ticks changed, so releasing it sends nothing to the model;
				// a docked page is still open, and its view releases it.
				if (shell.docked?.id !== note.id) await finishPage(note.id);
			} else {
				await updateNote(note.date, note.id, body);
			}
			await shell.refresh();
		} catch (e) {
			shell.showError(String(e));
		}
	}
</script>

<View back={shell.back} title={m.tasks_open()} detail={count ? String(count) : undefined}>
	{#if groups.length === 0}
		<p class="text-base text-neutral-600">{m.tasks_none()}</p>
	{:else}
		<!-- Laid out as the timeline is: when on the left, the tasks on the right. -->
		<ul>
			{#each groups as { note, body, open } (note.id)}
				<li class="-mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 px-3 py-3">
					<button
						type="button"
						onclick={() => void shell.openCited(note)}
						title={isPage(note) ? m.pages_open() : m.tasks_show_in_day()}
						class="cursor-pointer self-start pt-1 text-right font-mono text-xs leading-5 text-neutral-600 transition-colors hover:text-neutral-300"
					>
						<span class="block">{note.date}</span>{note.time}
					</button>
					<div class="min-w-0">
						{#if isPage(note)}
							<button
								type="button"
								onclick={() => void shell.openPage(note)}
								class="max-w-full cursor-pointer truncate text-sm leading-7 font-medium text-neutral-400 transition-colors hover:text-neutral-200"
							>
								{note.subject ?? m.pages_untitled()}
							</button>
						{/if}
						{#each open as task (task.at)}
							<Markdown
								text={body.slice(task.from, task.to)}
								ontoggle={() => toggle(note, task.at)}
								class="text-base leading-7 text-neutral-200"
							/>
						{/each}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</View>
