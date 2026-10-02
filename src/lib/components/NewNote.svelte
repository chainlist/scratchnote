<script lang="ts">
	import { tick } from 'svelte';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import SendHorizontalIcon from '@lucide/svelte/icons/send-horizontal';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import Recall from '#lib/components/Recall.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		onsave,
		onpage,
		onerror,
		centered = false,
		writing = $bindable(false)
	}: {
		/** Save the note to the day shown. Resolves true once saved; false keeps the editor open. */
		onsave: (body: string) => Promise<boolean>;
		/** Start a page on the day shown instead (SPEC 3.5). */
		onpage: () => void;
		/** A file could not be attached. */
		onerror: (message: string) => void;
		/** On an empty day, with no cards to line up under: the button sits centred and the editor spans the page. */
		centered?: boolean;
		/** The editor is open rather than the button. */
		writing?: boolean;
	} = $props();

	let saving = $state(false);
	let draft = $state('');
	let editor = $state<MarkdownEditor | null>(null);

	async function start() {
		writing = true;
		await tick();
		editor?.focus();
	}

	async function save() {
		if (saving) return;
		if (draft.trim() === '') {
			writing = false;
			return;
		}
		saving = true;
		const saved = await onsave(draft);
		saving = false;
		if (saved) {
			draft = '';
			writing = false;
		}
	}

	const action =
		'cursor-pointer rounded px-1.5 py-0.5 text-[0.625rem] tracking-wide text-neutral-400 uppercase hover:bg-neutral-800 hover:text-neutral-200 disabled:opacity-50';

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			writing = false;
		}
	}
</script>

<!-- Laid out like a note card so the button and the editor sit in the body
     column, under the notes above. -->
<div class={centered ? 'w-full' : '-mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 px-3 py-3'}>
	<div class={centered ? (writing ? 'text-left' : 'flex justify-center') : 'col-start-2 min-w-0'}>
		{#if writing}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="flex flex-col gap-2" onkeydown={onKeydown}>
				<MarkdownEditor
					bind:this={editor}
					bind:value={draft}
					placeholder={m.capture_placeholder()}
					label={m.note_body_label()}
					{onerror}
					class="-mx-2 max-h-[calc(16lh+0.5rem)] min-h-[calc(3lh+0.5rem)] w-[calc(100%+1rem)] rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-base leading-7 text-neutral-100 focus-within:border-neutral-600"
				/>
				<div class="flex items-center justify-end gap-1">
					<!-- Opening the old note would leave this one unsaved, so it is
					     only read here (SPEC 6.3). -->
					<Recall text={draft} class="mr-auto min-w-0 text-xs text-neutral-500">
						<span class="mr-auto text-[0.625rem] text-neutral-600">{m.note_edit_hint()}</span>
					</Recall>
					<button type="button" onclick={() => (writing = false)} class={action}
						>{m.common_cancel()}</button
					>
					<Button
						size="icon-sm"
						onclick={save}
						disabled={saving}
						aria-label={m.common_save()}
						title={m.common_save()}
					>
						<SendHorizontalIcon />
					</Button>
				</div>
			</div>
		{:else}
			<div class="flex items-center gap-3 {centered ? 'justify-center' : ''}">
				<button
					type="button"
					onclick={start}
					class="-mx-1.5 flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 text-sm text-neutral-600 hover:bg-neutral-900 hover:text-neutral-300"
				>
					<PlusIcon class="size-3.5" />{m.page_add_note()}
				</button>
				<button
					type="button"
					onclick={onpage}
					class="flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 text-sm text-neutral-600 hover:bg-neutral-900 hover:text-neutral-300"
				>
					<FileTextIcon class="size-3.5" />{m.pages_new()}
				</button>
			</div>
		{/if}
	</div>
</div>
