<script lang="ts">
	import type { Note } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { m } from '$lib/paraglide/messages';

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
			<Dialog.Title>{m.page_delete_title()}</Dialog.Title>
			<Dialog.Description>{m.page_delete_description()}</Dialog.Description>
		</Dialog.Header>
		{#if note}
			<p
				class="line-clamp-4 rounded-lg border bg-muted/40 px-3 py-2 text-sm whitespace-pre-wrap text-muted-foreground"
			>
				{note.body}
			</p>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (note = null)}>{m.common_cancel()}</Button>
			<Button variant="destructive" onclick={confirm} disabled={removing}>
				{removing ? m.page_deleting() : m.common_delete()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
