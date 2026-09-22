<script lang="ts">
	import { downloadModel, type ModelStatus, type ModelVariant } from '$lib/api';

	let { status }: { status: ModelStatus } = $props();

	let error = $state<string | null>(null);
	let starting = $state<ModelVariant | null>(null);

	const choices: { variant: ModelVariant; name: string; size: string; note: string }[] = [
		{
			variant: 'default',
			name: 'Default',
			size: '~2.5 GB',
			note: 'Qwen3 4B. Better labels, wants more RAM.'
		},
		{
			variant: 'light',
			name: 'Light',
			size: '~1.1 GB',
			note: 'Qwen3 1.7B. Quicker, kinder to a small machine.'
		}
	];

	async function start(variant: ModelVariant) {
		starting = variant;
		error = null;
		try {
			await downloadModel(variant);
		} catch (e) {
			error = String(e);
		} finally {
			starting = null;
		}
	}
</script>

<section class="mb-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
	{#if status.state === 'downloading'}
		<h2 class="mb-1 text-sm font-medium">Downloading the model</h2>
		<p class="mb-3 text-xs text-neutral-400">
			Capture keeps working. Notes stay pending until this finishes.
		</p>
		<div class="h-1.5 overflow-hidden rounded-full bg-neutral-800">
			<div
				class="h-full rounded-full bg-neutral-300 transition-[width] duration-300"
				style="width: {status.percent ?? 0}%"
			></div>
		</div>
		<p class="mt-2 text-right font-mono text-[11px] text-neutral-500">{status.percent ?? 0}%</p>
	{:else}
		<h2 class="mb-1 text-sm font-medium">No model installed</h2>
		<p class="mb-3 text-xs text-neutral-400">
			Notes are saved and searchable without one. A model adds a subject, a summary and tags.
		</p>

		<div class="flex flex-wrap gap-2">
			{#each choices as choice (choice.variant)}
				<button
					type="button"
					onclick={() => start(choice.variant)}
					disabled={starting !== null}
					class="flex-1 cursor-pointer rounded border border-neutral-700 p-3 text-left
						hover:border-neutral-500 disabled:cursor-default disabled:opacity-50"
				>
					<span class="block text-sm text-neutral-100">
						{choice.name}
						<span class="font-mono text-[11px] text-neutral-500">{choice.size}</span>
					</span>
					<span class="block text-xs text-neutral-400">{choice.note}</span>
				</button>
			{/each}
		</div>

		{#if error}
			<p class="mt-3 text-xs text-red-400">{error}</p>
		{/if}
	{/if}
</section>
