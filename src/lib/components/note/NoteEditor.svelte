<script lang="ts">
	import InlineError from '#lib/components/InlineError.svelte';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import Recall from '#lib/components/note/Recall.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { withMention } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		value = $bindable(''),
		placeholder,
		saving,
		failure,
		initial,
		exclude,
		onsave,
		onclose,
		onerror
	}: {
		/** The note's text as it is being typed. */
		value?: string;
		placeholder?: string;
		/** A save is under way, so Save waits. */
		saving: boolean;
		/** The last save's error, shown under the editor until a save works. */
		failure?: string;
		/** The text the editor opened with, which recall does not look for. */
		initial?: string;
		/** The note being edited, which recall does not find. */
		exclude?: string;
		/** Save, by Mod+Enter or the button. */
		onsave: (fromKeyboard: boolean) => void;
		/** Close with the draft kept, by Esc or the button. */
		onclose: (fromKeyboard: boolean) => void;
		/** A file could not be attached. */
		onerror: (message: string) => void;
	} = $props();

	let editor = $state<MarkdownEditor | null>(null);
	let box = $state<HTMLElement>();

	export function focus() {
		editor?.focus();
	}

	/** Bring the editor into view, as it may open below the window's edge. */
	export function reveal() {
		box?.scrollIntoView({ block: 'nearest' });
	}

	const action =
		'cursor-pointer rounded px-1.5 py-0.5 text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 disabled:opacity-50';

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			onsave(true);
		} else if (event.key === 'Escape' && !event.defaultPrevented) {
			// The editor takes one first that closes the names `@` offers.
			event.preventDefault();
			onclose(true);
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div bind:this={box} class="flex flex-col gap-2" onkeydown={onKeydown}>
	<MarkdownEditor
		bind:this={editor}
		bind:value
		{placeholder}
		label={m.note_body_label()}
		{onerror}
		class="-mx-2 max-h-[calc(16lh+0.5rem)] min-h-[calc(3lh+0.5rem)] w-[calc(100%+1rem)] rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-base leading-7 text-neutral-100 focus-within:border-neutral-600"
	/>
	<div class="flex items-center justify-end gap-1">
		<!-- Opening the old note would leave this one unsaved, so it is
		     only read here (SPEC 6.3). -->
		<Recall
			text={value}
			{initial}
			{exclude}
			onmention={(name) => (value = withMention(value, name))}
			class="mr-auto min-w-0 text-xs text-meta"
		>
			<span class="mr-auto text-xs text-meta">{m.note_new_hint()}</span>
		</Recall>
		<!-- Closing keeps the draft, so the button says close, not cancel. -->
		<button type="button" onclick={() => onclose(false)} class={action}>{m.common_close()}</button>
		<Button size="sm" onclick={() => onsave(false)} disabled={saving}>
			{m.common_save()}
		</Button>
	</div>
	{#if failure !== undefined}
		<InlineError message={m.error_save_note()} detail={failure} />
	{/if}
</div>
