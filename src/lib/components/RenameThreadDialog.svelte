<script lang="ts">
	import type { Thread } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		thread = $bindable(),
		onconfirm
	}: {
		/** The thread to rename; null closes the dialog. */
		thread: Thread | null;
		/** Resolves to an error to show, or null once renamed. */
		onconfirm: (thread: Thread, title: string) => Promise<string | null>;
	} = $props();

	let title = $state('');
	let input = $state<HTMLInputElement | null>(null);
	let working = $state(false);
	let error = $state<string | null>(null);

	// The field starts on the user's title, empty while there is none.
	let loaded: string | null = null;
	$effect(() => {
		const id = thread?.id ?? null;
		if (id === loaded) return;
		loaded = id;
		title = thread?.named ? (thread.title ?? '') : '';
		error = null;
	});

	async function confirm(event?: SubmitEvent) {
		event?.preventDefault();
		if (!thread || working) return;
		working = true;
		error = await onconfirm(thread, title);
		working = false;
		if (error === null) thread = null;
	}
</script>

<Dialog.Root
	open={thread !== null}
	onOpenChange={(open) => {
		if (!open) thread = null;
	}}
>
	<!-- The title field takes the focus, its text selected. -->
	<Dialog.Content
		class="sm:max-w-[min(28rem,calc(100%-2rem))]"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			input?.select();
		}}
	>
		<form class="flex flex-col gap-4" onsubmit={confirm}>
			<Dialog.Header>
				<Dialog.Title>{m.thread_rename_title()}</Dialog.Title>
				<Dialog.Description>{m.thread_rename_description()}</Dialog.Description>
			</Dialog.Header>
			<div class="flex flex-col gap-2">
				<Label for="thread-title">{m.thread_title_label()}</Label>
				<Input
					id="thread-title"
					bind:ref={input}
					bind:value={title}
					placeholder={m.thread_untitled()}
					maxlength={120}
				/>
			</div>
			{#if error}
				<p class="text-sm text-destructive">{error}</p>
			{/if}
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (thread = null)}
					>{m.common_cancel()}</Button
				>
				<Button type="submit" disabled={working}>{m.common_save()}</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
