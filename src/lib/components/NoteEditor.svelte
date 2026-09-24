<script lang="ts">
	import { splitCategory, type Note, type NoteEdit } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '$lib/paraglide/messages';

	let {
		note = $bindable(),
		categories,
		tags,
		onsave,
		ondelete
	}: {
		/** The note being edited; null closes the editor. */
		note: Note | null;
		/** Every category on the list, in its order. */
		categories: string[];
		/** Every tag in use with its count, most used first, offered as suggestions. */
		tags: [string, number][];
		/** Resolves to an error to show, or null once saved. */
		onsave: (note: Note, edit: NoteEdit) => Promise<string | null>;
		/** Ask to delete; the page confirms. */
		ondelete: (note: Note) => void;
	} = $props();

	/** Suggestions shown at once. */
	const LIMIT = 6;

	let body = $state('');
	let subject = $state('');
	let category = $state('');
	let chosen = $state<string[]>([]);
	let tagDraft = $state('');
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
		const [current, rest] = splitCategory(categories, note.tags);
		body = note.body;
		subject = note.subject ?? '';
		category = current;
		chosen = [...rest];
		tagDraft = '';
		newCategory = null;
		error = null;
	});

	/** What a typed tag or category will be saved as; the backend cleans it the same way. */
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

	const suggestions = $derived.by(() => {
		const prefix = clean(tagDraft);
		if (!prefix) return [];
		const starts: string[] = [];
		const contains: string[] = [];
		for (const [name] of tags) {
			if (chosen.includes(name) || name === category) continue;
			if (name.startsWith(prefix)) starts.push(name);
			else if (name.includes(prefix)) contains.push(name);
		}
		return [...starts, ...contains].slice(0, LIMIT);
	});

	function addTag(raw: string) {
		const tag = clean(raw);
		tagDraft = '';
		if (!tag || tag === category || chosen.includes(tag)) return;
		chosen = [...chosen, tag];
	}

	function removeTag(tag: string) {
		chosen = chosen.filter((t) => t !== tag);
	}

	function onTagKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ',' || event.key === ' ') {
			if (tagDraft.trim() === '') return;
			event.preventDefault();
			addTag(tagDraft);
		} else if (event.key === 'Backspace' && tagDraft === '' && chosen.length > 0) {
			chosen = chosen.slice(0, -1);
		}
	}

	function pickCategory(name: string) {
		category = category === name ? '' : name;
		// A category is not also one of the tags.
		chosen = chosen.filter((t) => t !== category);
	}

	function commitNewCategory() {
		const name = clean(newCategory ?? '');
		newCategory = null;
		if (name) {
			category = name;
			chosen = chosen.filter((t) => t !== name);
		}
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
		// A half-typed tag counts; leaving it behind would surprise.
		if (tagDraft.trim() !== '') addTag(tagDraft);
		saving = true;
		error = await onsave(note, { body, subject, category, tags: chosen });
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
			<Label for="note-body">{m.editor_text()}</Label>
			<textarea
				id="note-body"
				bind:value={body}
				rows={Math.min(12, Math.max(4, body.split('\n').length))}
				spellcheck="false"
				class="w-full resize-y rounded-lg border border-input bg-transparent px-2.5 py-1.5 text-sm leading-6 transition-colors outline-none focus-visible:border-ring dark:bg-input/30"
			></textarea>
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
						{name}
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

		<div class="flex flex-col gap-2">
			<Label for="note-tags">{m.editor_tags()}</Label>
			<div
				class="flex min-h-8 flex-wrap items-center gap-1.5 rounded-lg border border-input px-1.5 py-1 focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:bg-input/30"
			>
				{#each chosen as tag (tag)}
					<span
						class="inline-flex h-6 items-center gap-0.5 rounded-md bg-muted pr-0.5 pl-2 font-mono text-xs"
					>
						#{tag}
						<button
							type="button"
							onclick={() => removeTag(tag)}
							aria-label={m.editor_remove_tag({ tag })}
							class="cursor-pointer rounded p-0.5 text-muted-foreground hover:text-foreground"
						>
							<XIcon class="size-3" />
						</button>
					</span>
				{/each}
				<input
					id="note-tags"
					bind:value={tagDraft}
					onkeydown={onTagKeydown}
					placeholder={chosen.length === 0 ? m.editor_add_tag() : ''}
					autocomplete="off"
					spellcheck="false"
					class="h-6 min-w-24 flex-1 bg-transparent px-1 font-mono text-xs outline-none placeholder:text-muted-foreground"
				/>
			</div>
			{#if suggestions.length > 0}
				<div class="flex flex-wrap gap-1.5">
					{#each suggestions as name (name)}
						<button
							type="button"
							onclick={() => addTag(name)}
							class="{chip} border-dashed border-border text-muted-foreground hover:text-foreground"
						>
							#{name}
						</button>
					{/each}
				</div>
			{/if}
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
