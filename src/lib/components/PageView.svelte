<script lang="ts">
	import { onDestroy, onMount, tick } from 'svelte';
	import type { Note } from '#lib/api.js';
	import MarkdownEditor from '#lib/components/editor/MarkdownEditor.svelte';
	import DayAhead from '#lib/components/note/DayAhead.svelte';
	import NoteActionItems from '#lib/components/note/NoteActionItems.svelte';
	import Recall from '#lib/components/note/Recall.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import { withMention } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { PageSession } from '#lib/components/page-session.svelte.js';

	let {
		id,
		date,
		oncreated,
		ondelete,
		onsimilar,
		onmove,
		onopennote,
		own = false
	}: {
		/** The page to open, or null for a new one. */
		id: string | null;
		/** The day a new page goes on. */
		date: string;
		/** A new page got its title and was saved. */
		oncreated: (page: Note) => void;
		/** Ask to delete; the page confirms. */
		ondelete: (page: Note) => void;
		/** Left out without the embedding model. */
		onsimilar?: (page: Note) => void;
		/** Ask to move it to another space, once its text is saved. Left out with one space. */
		onmove?: (page: Note) => void;
		/** Show an old note the text is about, where it is. */
		onopennote?: (note: Note) => void;
		/** Open as a view of its own, not in the dock: its title is then the
		 *  view's heading and the window's title. */
		own?: boolean;
	} = $props();

	let editor = $state<MarkdownEditor | null>(null);
	let titleInput = $state<HTMLInputElement | null>(null);
	const session = new PageSession({
		get date() {
			return date;
		},
		oncreated: (page) => oncreated(page),
		editingTitle: () => document.activeElement === titleInput
	});

	onMount(() => {
		void (async () => {
			await session.open(id);
			await tick();
			if (id === null) titleInput?.focus();
			else editor?.focus();
		})();
		// An edit in another editor.
		return session.listen();
	});

	onDestroy(() => session.close());

	/** Text handed over from the capture window, after what is typed. */
	export function addText(text: string) {
		session.addText(text);
	}

	function onTitleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			// Moving on blurs the field, which saves the title.
			editor?.focus();
		} else if (event.key === 'Escape' && session.page) {
			event.preventDefault();
			session.title = session.page.subject ?? '';
			editor?.focus();
		}
	}

	function onKeydown(event: KeyboardEvent) {
		// Saving is automatic; this only makes it now.
		if (event.key === 's' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			void session.saveNow();
		}
	}
</script>

<svelte:head>
	{#if own}<title>{session.title.trim() || m.pages_untitled()} · Scratchnote</title>{/if}
</svelte:head>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="flex flex-col gap-4 pb-16" onkeydown={onKeydown}>
	<!-- The title is a field; the view's heading, for a screen reader's list
	     of them, says the same. -->
	{#if own}<h1 class="sr-only">{session.title.trim() || m.pages_untitled()}</h1>{/if}
	<div class="flex items-start gap-2">
		<input
			bind:this={titleInput}
			bind:value={session.title}
			onblur={() => {
				// Switching to another app blurs the field too; a title half typed
				// then must not become a file name. It saves once focus moves on
				// inside the window, or on Enter.
				if (document.hasFocus()) void session.saveTitle();
			}}
			onkeydown={onTitleKeydown}
			placeholder={m.pages_title_placeholder()}
			aria-label={m.pages_title_label()}
			disabled={session.loading}
			class="min-w-0 flex-1 border-b border-transparent bg-transparent text-2xl font-semibold tracking-tight text-neutral-100 outline-none placeholder:text-meta focus-visible:border-ring"
		/>
		{#if session.page}
			{@const current = session.page}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					aria-label={m.note_actions()}
					title={m.note_actions()}
					class="mt-0.5 flex size-7 cursor-pointer items-center justify-center rounded text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200"
				>
					<EllipsisIcon class="size-4" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-max max-w-80 min-w-52">
					<NoteActionItems
						note={current}
						{onsimilar}
						onmove={onmove && (() => void session.move(onmove))}
						{ondelete}
					/>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
	</div>

	<div class="-mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-meta">
		{#if session.page}<span class="font-mono">{session.page.date} {session.page.time}</span>{/if}
		<span>{m.pages_words({ count: session.words })}</span>
		{#if session.page?.on}<DayAhead on={session.page.on} />{/if}
		<span
			class={session.error ? 'text-destructive' : ''}
			title={session.error?.detail}
			aria-live="polite">{session.status}</span
		>
		{#if !session.loading}
			<Recall
				text={session.body}
				initial={session.openedWith}
				exclude={session.page?.id}
				onopen={onopennote}
				onmention={(name) => (session.body = withMention(session.body, name))}
				class="max-w-full"
			/>
		{/if}
	</div>

	{#if !session.loading}
		<MarkdownEditor
			bind:this={editor}
			bind:value={session.body}
			placeholder={m.pages_body_placeholder()}
			label={m.pages_body_label()}
			onerror={(message) => (session.error = { what: message })}
			class="min-h-[50vh] text-base leading-7 text-neutral-200"
			toolbarClass="sticky top-0 z-10 bg-neutral-950"
		/>
	{/if}
</div>
