<script lang="ts">
	import { isPage, type Note } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import NotePreview from '#lib/components/note/NotePreview.svelte';
	import { m } from '#lib/paraglide/messages.js';

	let {
		note = $bindable(),
		onconfirm
	}: {
		/** The note waiting on the confirmation; null closes it. */
		note: Note | null;
		/** Deletes the note. The dialog closes once it settles, failed or not. */
		onconfirm: (note: Note) => Promise<void>;
	} = $props();

	let removing = $state(false);

	async function confirm() {
		if (!note || removing) return;
		removing = true;
		try {
			await onconfirm(note);
		} finally {
			removing = false;
			note = null;
		}
	}
</script>

<Dialog.Root
	open={note !== null}
	onOpenChange={(open) => {
		if (!open) note = null;
	}}
>
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			{#if note?.missing}
				<Dialog.Title>{m.pages_remove_stub_title()}</Dialog.Title>
				<Dialog.Description>{m.pages_remove_stub_description()}</Dialog.Description>
			{:else if note && isPage(note)}
				<Dialog.Title>{m.pages_delete_title()}</Dialog.Title>
				<Dialog.Description>{m.pages_delete_description()}</Dialog.Description>
			{:else}
				<Dialog.Title>{m.page_delete_title()}</Dialog.Title>
				<Dialog.Description>{m.page_delete_description()}</Dialog.Description>
			{/if}
		</Dialog.Header>
		{#if note}<NotePreview {note} clamp="max-h-[4lh] overflow-hidden" />{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (note = null)}>{m.common_cancel()}</Button>
			<Button variant="destructive" onclick={confirm} disabled={removing}>
				{removing ? m.page_deleting() : m.common_delete()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
