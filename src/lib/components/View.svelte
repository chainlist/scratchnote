<script lang="ts">
	import type { Snippet } from 'svelte';
	import ViewHeader from '#lib/components/ViewHeader.svelte';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell, type ViewTitle } from '#lib/shell.svelte.js';

	let {
		key,
		fill = false,
		children,
		...header
	}: ViewTitle & {
		/** Changes when another of the same view opens, such as the next day,
		 *  which then rises into place as a view of its own does. */
		key?: unknown;
		/** The whole main area under the title rather than the reading
		 *  column, with a set height, as a plugin's page may ask. */
		fill?: boolean;
		children: Snippet;
	} = $props();

	const shell = getShell();

	/** A view opens at its top. */
	function fromTop(node: HTMLElement) {
		node.closest('main')?.scrollTo({ top: 0 });
	}
</script>

<!-- A view's column: its title, what the app has to say, and the view. -->
<div class={fill ? 'flex h-full w-full flex-col' : 'mx-auto w-full max-w-3xl'}>
	<ViewHeader {...header} />

	<!-- What failed in plain words, another try where one makes sense, and
	     the error as it came under Details. Read out as it appears. -->
	{#if shell.error}
		{@const retry = shell.errorRetry}
		<Alert.Root variant="destructive" class="mb-6">
			<CircleAlertIcon />
			<Alert.Description class="flex flex-col items-start gap-2">
				<span>{shell.error}</span>
				{#if retry || shell.errorDetail}
					<div class="flex flex-wrap items-center gap-3">
						{#if retry}
							<Button
								variant="outline"
								size="sm"
								onclick={() => {
									shell.error = null;
									void retry();
								}}
							>
								<RotateCwIcon />{m.error_retry()}
							</Button>
						{/if}
						{#if shell.errorDetail}
							<details class="text-xs text-meta">
								<summary class="cursor-pointer">{m.error_details()}</summary>
								<p class="mt-1 font-mono break-all whitespace-pre-wrap select-text">
									{shell.errorDetail}
								</p>
							</details>
						{/if}
					</div>
				{/if}
			</Alert.Description>
			<Alert.Action>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => (shell.error = null)}
					aria-label={m.error_dismiss()}
					title={m.error_dismiss()}
				>
					<XIcon />
				</Button>
			</Alert.Action>
		</Alert.Root>
	{/if}

	{#key key}
		<div class={['page-in', fill && 'min-h-0 flex-1']} {@attach fromTop}>
			{@render children()}
		</div>
	{/key}
</div>
