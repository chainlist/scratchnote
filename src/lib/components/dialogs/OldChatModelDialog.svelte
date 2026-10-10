<script lang="ts">
	import { onMount } from 'svelte';
	import { oldChatModel, removeOldChatModel } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { readJson, writeJson } from '#lib/storage.js';

	let {
		waiting,
		onerror
	}: {
		/** Another dialog is up, such as What's new after an update: this one
		 *  waits until it is closed. */
		waiting: boolean;
		/** The files could not be removed. */
		onerror: (message: string) => void;
	} = $props();

	/** Set on this computer once the user chose to keep the old model, so they
	 *  are not asked again. */
	const KEPT = 'scratchnote.oldChatModelKept';

	/** How many bytes the old model takes; null once nothing is to be asked. */
	let bytes = $state<number | null>(null);
	let removing = $state(false);

	onMount(() => {
		// Without storage, it asks each launch.
		if (readJson(KEPT)) return;
		oldChatModel()
			.then((found) => (bytes = found?.bytes ?? null))
			.catch((e) => onerror(String(e)));
	});

	/** In gigabytes, or megabytes for a download left half done, as the
	 *  language writes them. */
	const size = $derived.by(() => {
		if (bytes === null) return '';
		const [value, unit] = bytes >= 1e9 ? [bytes / 1e9, 'gigabyte'] : [bytes / 1e6, 'megabyte'];
		return new Intl.NumberFormat(getLocale(), {
			style: 'unit',
			unit,
			maximumFractionDigits: 1
		}).format(value);
	});

	function keep() {
		writeJson(KEPT, true);
		bytes = null;
	}

	async function remove() {
		if (removing) return;
		removing = true;
		try {
			await removeOldChatModel();
			bytes = null;
		} catch (e) {
			onerror(m.error_old_model({ reason: String(e) }));
		} finally {
			removing = false;
		}
	}
</script>

<!-- Closed any other way than its buttons, it asks again at the next launch. -->
<Dialog.Root
	open={bytes !== null && !waiting}
	onOpenChange={(open) => {
		if (!open) bytes = null;
	}}
>
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title>{m.old_model_title()}</Dialog.Title>
			<Dialog.Description>{m.old_model_body({ size })}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={keep}>{m.old_model_keep()}</Button>
			<Button onclick={remove} disabled={removing}>
				{removing ? m.old_model_removing() : m.old_model_remove()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
