<script lang="ts">
	import { isPage, type Note, type SpacesView } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import Markdown from '$lib/components/Markdown.svelte';
	import { m } from '$lib/paraglide/messages';

	let {
		note = $bindable(),
		spaces,
		onconfirm
	}: {
		/** The note or page to move; null closes the dialog. */
		note: Note | null;
		/** Every space; the note is in the open one and goes to another. */
		spaces: SpacesView | null;
		/** Resolves to an error to show, or null once moved. */
		onconfirm: (note: Note, space: string) => Promise<string | null>;
	} = $props();

	const others = $derived(spaces?.spaces.filter((space) => space.name !== spaces.active) ?? []);

	let target = $state('');
	let trigger = $state<HTMLElement | null>(null);
	let working = $state(false);
	let error = $state<string | null>(null);
	/** What the dialog shows, kept as it was while it fades out. */
	let shown = $state<Note | null>(null);

	// Each opening starts on the first other space. A space renamed or
	// deleted from elsewhere meanwhile is no longer one to move to.
	let loaded: string | null = null;
	$effect(() => {
		if (!note) {
			loaded = null;
			return;
		}
		shown = note;
		if (note.id !== loaded) {
			loaded = note.id;
			target = '';
			error = null;
		}
		if (!others.some((space) => space.name === target)) target = others[0]?.name ?? '';
	});

	async function confirm(event?: SubmitEvent) {
		event?.preventDefault();
		if (!note || working || !target) return;
		working = true;
		error = await onconfirm(note, target);
		working = false;
		if (error === null) note = null;
	}
</script>

<Dialog.Root
	open={note !== null}
	onOpenChange={(open) => {
		if (!open) note = null;
	}}
>
	<!-- The space picker takes the focus, on the first other space. -->
	<Dialog.Content
		class="sm:max-w-[min(28rem,calc(100%-2rem))]"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			trigger?.focus();
		}}
	>
		<form class="flex flex-col gap-4" onsubmit={confirm}>
			<Dialog.Header>
				<Dialog.Title>
					{shown && isPage(shown) ? m.move_page_title() : m.move_note_title()}
				</Dialog.Title>
				<Dialog.Description>
					{m.move_description({ space: spaces?.active ?? '' })}
				</Dialog.Description>
			</Dialog.Header>
			{#if shown}
				<div class="rounded-lg border bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
					{#if isPage(shown)}
						<p class="truncate font-medium text-foreground">
							{shown.subject ?? m.pages_untitled()}
						</p>
					{/if}
					{#if shown.body}
						<Markdown text={shown.body} links={false} class="line-clamp-2" />
					{/if}
				</div>
			{/if}
			<div class="flex flex-col gap-2">
				<Label for="move-space">{m.move_space_label()}</Label>
				<Select.Root type="single" bind:value={target}>
					<Select.Trigger id="move-space" bind:ref={trigger} class="w-full">
						<span class="truncate">{target}</span>
					</Select.Trigger>
					<Select.Content>
						{#each others as space (space.name)}
							<Select.Item value={space.name} label={space.name} />
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			{#if error}
				<p class="text-sm text-destructive">{error}</p>
			{/if}
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (note = null)}
					>{m.common_cancel()}</Button
				>
				<Button type="submit" disabled={working || !target}>
					{working ? m.move_moving() : m.move_confirm()}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
