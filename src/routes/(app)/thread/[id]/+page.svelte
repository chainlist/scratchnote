<script lang="ts">
	import { renameThread } from '$lib/api';
	import NoteList from '$lib/components/NoteList.svelte';
	import View from '$lib/components/View.svelte';
	import { shortDay } from '$lib/components/ViewHeader.svelte';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';

	let { data, params } = $props();

	const shell = getShell();
	const thread = $derived(data.found?.thread ?? null);

	/** The title being typed, while the field is open. */
	let renaming = $state<string | null>(null);

	async function rename() {
		const title = renaming;
		renaming = null;
		if (title === null || !thread || title.trim() === (thread.named ? thread.title : '')) return;
		try {
			await renameThread(thread.id, title);
		} catch (e) {
			shell.showError(String(e));
		}
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			void rename();
		} else if (event.key === 'Escape') {
			// Only the field closes, not the view.
			event.preventDefault();
			event.stopPropagation();
			renaming = null;
		}
	}
</script>

<!-- One thread's notes, oldest first, so they read as it went (SPEC 6.4). -->
<View
	back={shell.back}
	title={thread?.title ?? m.thread_untitled()}
	detail={thread
		? m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })
		: undefined}
	key={params.id}
>
	{#if thread && data.found}
		<div class="mb-6 flex min-h-7 items-center gap-3 text-sm text-neutral-500">
			{#if renaming === null}
				<button
					type="button"
					onclick={() => (renaming = thread.named ? (thread.title ?? '') : '')}
					class="-mx-1.5 flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 hover:bg-neutral-900 hover:text-neutral-200"
				>
					<PencilIcon class="size-3.5" />{m.thread_rename()}
				</button>
			{:else}
				<!-- svelte-ignore a11y_autofocus -->
				<input
					bind:value={renaming}
					onkeydown={onKeydown}
					onblur={() => void rename()}
					autofocus
					placeholder={thread.title ?? m.thread_untitled()}
					aria-label={m.thread_title_label()}
					class="h-7 w-72 rounded-md border border-neutral-700 bg-transparent px-2 text-neutral-100 outline-none focus:border-neutral-500"
				/>
				<span class="text-xs text-neutral-600">{m.thread_rename_hint()}</span>
			{/if}
		</div>
		<NoteList
			notes={data.found.notes}
			empty=""
			showDate
			threadLine={false}
			{...shell.cardActions}
		/>
	{:else}
		<p class="text-base text-neutral-600">{m.thread_gone()}</p>
	{/if}
</View>
