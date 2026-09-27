<script lang="ts">
	import type { Note, NoteEdit } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { categoryLabel } from '$lib/categories';
	import { m } from '$lib/paraglide/messages';

	let {
		note = $bindable(),
		categories,
		onsave,
		ondelete
	}: {
		/** The note being edited; null closes the editor. */
		note: Note | null;
		/** Every category on the list, in its order. */
		categories: string[];
		/** Resolves to an error to show, or null once saved. */
		onsave: (note: Note, edit: NoteEdit) => Promise<string | null>;
		/** Ask to delete; the page confirms. */
		ondelete: (note: Note) => void;
	} = $props();

	let body = $state('');
	let subject = $state('');
	let category = $state('');
	let newCategory = $state<string | null>(null);
	let saving = $state(false);
	let error = $state<string | null>(null);

	// Loaded afresh each time a note is opened, so a cancelled edit leaves
	// nothing behind for the next one.
	let loaded: string | null = null;
	$effect(() => {
		const id = note?.id ?? null;
		if (id === loaded) return;
		loaded = id;
		if (!note) return;
		body = note.body;
		subject = note.subject ?? '';
		category = note.category ?? '';
		newCategory = null;
		error = null;
	});

	/** What a typed category will be saved as; the backend cleans it the same way. */
	const clean = (raw: string) =>
		raw
			.trim()
			.replace(/^#+/, '')
			.toLowerCase()
			.replace(/[\s_]+/g, '-')
			.replace(/[^\p{L}\p{N}-]/gu, '')
			.replace(/-+/g, '-')
			.replace(/^-|-$/g, '');

	// A category typed in stays on offer while the editor is open, even
	// before it is saved onto the list.
	const offered = $derived(
		category && !categories.includes(category) ? [...categories, category] : categories
	);

	function pickCategory(name: string) {
		category = category === name ? '' : name;
	}

	function commitNewCategory() {
		const name = clean(newCategory ?? '');
		newCategory = null;
		if (name) category = name;
	}

	function onNewCategoryKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			commitNewCategory();
		} else if (event.key === 'Escape') {
			// Only the new-category field closes, not the whole editor.
			event.preventDefault();
			event.stopPropagation();
			newCategory = null;
		}
	}

	async function save() {
		if (!note || saving) return;
		if (body.trim() === '') {
			error = m.editor_empty();
			return;
		}
		saving = true;
		error = await onsave(note, { body, subject, category });
		saving = false;
		if (error === null) note = null;
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		}
	}

	const chip =
		'inline-flex h-6 cursor-pointer items-center gap-1 rounded-md border px-2 font-mono text-xs transition-colors';
</script>

<Dialog.Root
	open={note !== null}
	onOpenChange={(open) => {
		if (!open) note = null;
	}}
>
	<Dialog.Content class="gap-5 sm:max-w-[min(32rem,calc(100%-2rem))]" onkeydown={onKeydown}>
		<Dialog.Header>
			<Dialog.Title>{m.editor_title()}</Dialog.Title>
			<Dialog.Description>
				{#if note}{m.editor_when({ date: note.date, time: note.time })}{/if}
				{m.editor_description()}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-2">
			<span class="text-sm font-medium">{m.editor_text()}</span>
			<MarkdownEditor
				bind:value={body}
				label={m.editor_text()}
				class="max-h-[calc(12lh+0.75rem)] min-h-[calc(4lh+0.75rem)] w-full rounded-lg border border-input bg-transparent px-2.5 py-1.5 text-sm leading-6 transition-colors focus-within:border-ring dark:bg-input/30"
			/>
		</div>

		<div class="flex flex-col gap-2">
			<Label for="note-subject">{m.editor_subject()}</Label>
			<Input id="note-subject" bind:value={subject} placeholder={m.editor_untitled()} />
		</div>

		<div class="flex flex-col gap-2">
			<span class="text-sm font-medium">{m.editor_category()}</span>
			<div class="flex flex-wrap gap-1.5" role="radiogroup" aria-label={m.editor_category()}>
				{#each offered as name (name)}
					<button
						type="button"
						role="radio"
						aria-checked={category === name}
						onclick={() => pickCategory(name)}
						class="{chip} {category === name
							? 'border-primary bg-primary text-primary-foreground'
							: 'border-border text-muted-foreground hover:border-foreground/30 hover:text-foreground'}"
					>
						{categoryLabel(name)}
					</button>
				{/each}
				{#if newCategory === null}
					<button
						type="button"
						onclick={() => (newCategory = '')}
						class="{chip} border-dashed border-border text-muted-foreground hover:text-foreground"
					>
						<PlusIcon class="size-3" />{m.editor_new()}
					</button>
				{:else}
					<!-- svelte-ignore a11y_autofocus -->
					<input
						bind:value={newCategory}
						onkeydown={onNewCategoryKeydown}
						onblur={commitNewCategory}
						autofocus
						aria-label={m.editor_new_category_label()}
						placeholder={m.editor_new_category_placeholder()}
						class="h-6 w-32 rounded-md border border-ring bg-transparent px-2 font-mono text-xs outline-none"
					/>
				{/if}
			</div>
			<p class="text-xs text-muted-foreground">
				{m.editor_uncategorised_hint()}
			</p>
		</div>

		{#if error}
			<p class="text-sm text-destructive">{error}</p>
		{/if}

		<Dialog.Footer class="flex-row items-center sm:justify-between">
			<Button variant="ghost" class="text-destructive" onclick={() => note && ondelete(note)}>
				<Trash2Icon />{m.common_delete()}
			</Button>
			<div class="flex gap-2">
				<Button variant="outline" onclick={() => (note = null)}>{m.common_cancel()}</Button>
				<Button onclick={save} disabled={saving}
					>{saving ? m.common_saving() : m.common_save()}</Button
				>
			</div>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
