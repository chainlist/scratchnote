<script lang="ts">
	import {
		downloadModel,
		getSettings,
		MODEL_CHOICES,
		setSettings,
		type ModelStatus,
		type ModelVariant
	} from '$lib/api';
	import { m } from '$lib/paraglide/messages';

	let { status }: { status: ModelStatus } = $props();

	let error = $state<string | null>(null);
	let starting = $state<ModelVariant | null>(null);

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

	/** For a machine too small for either model. Settings can turn it back on. */
	async function skip() {
		error = null;
		try {
			// The backend ignores the view's extra activeRoot.
			await setSettings({ ...(await getSettings()), modelEnabled: false });
		} catch (e) {
			error = String(e);
		}
	}
</script>

<section class="mb-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
	{#if status.state === 'downloading'}
		<h2 class="mb-1 text-sm font-medium">{m.onboarding_downloading_title()}</h2>
		<p class="mb-3 text-xs text-neutral-400">
			{m.onboarding_downloading_body()}
		</p>
		<div class="h-1.5 overflow-hidden rounded-full bg-neutral-800">
			<div
				class="h-full rounded-full bg-neutral-300 transition-[width] duration-300"
				style="width: {status.percent ?? 0}%"
			></div>
		</div>
		<p class="mt-2 text-right font-mono text-[0.6875rem] text-neutral-500">
			{status.percent ?? 0}%
		</p>
	{:else}
		<h2 class="mb-1 text-sm font-medium">{m.status_absent()}</h2>
		<p class="mb-3 text-xs text-neutral-400">
			{m.onboarding_absent_body()}
		</p>

		<div class="flex flex-wrap gap-2">
			{#each MODEL_CHOICES as choice (choice.variant)}
				<button
					type="button"
					onclick={() => start(choice.variant)}
					disabled={starting !== null}
					class="flex-1 cursor-pointer rounded border border-neutral-700 p-3 text-left
						hover:border-neutral-500 disabled:cursor-default disabled:opacity-50"
				>
					<span class="block text-sm text-neutral-100">
						{choice.name()}
						<span class="font-mono text-[0.6875rem] text-neutral-500">{choice.size}</span>
					</span>
					<span class="block text-xs text-neutral-400">{choice.note()}</span>
				</button>
			{/each}
		</div>

		<button
			type="button"
			onclick={skip}
			disabled={starting !== null}
			class="mt-3 cursor-pointer text-xs text-neutral-500 hover:text-neutral-300
				disabled:cursor-default disabled:opacity-50"
		>
			{m.onboarding_skip()}
		</button>

		{#if error}
			<p class="mt-3 text-xs text-red-400">{error}</p>
		{/if}
	{/if}
</section>
