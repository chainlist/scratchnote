<script lang="ts">
	import { MODEL_CHOICES, type Hardware } from '#lib/api.js';
	import { group, hint } from '#lib/components/settings/styles.js';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';

	/** Null while the probe runs. */
	let { hardware }: { hardware: Hardware | null } = $props();

	const gigabytes = (bytes: number) =>
		new Intl.NumberFormat(getLocale(), { maximumFractionDigits: 0 }).format(bytes / 2 ** 30);

	const gpus = $derived(
		hardware?.gpus.map((gpu) =>
			gpu.integrated
				? m.onboarding_machine_shared({ name: gpu.name })
				: gpu.memory > 0
					? m.onboarding_machine_dedicated({ name: gpu.name, size: gigabytes(gpu.memory) })
					: gpu.name
		) ?? []
	);

	const rows = $derived(
		hardware
			? [
					{
						label: m.onboarding_machine_gpu(),
						value: gpus.length ? gpus.join(', ') : m.onboarding_machine_no_gpu()
					},
					{
						label: m.onboarding_machine_memory(),
						value:
							hardware.memory > 0
								? m.onboarding_machine_size({ size: gigabytes(hardware.memory) })
								: m.onboarding_machine_unknown()
					},
					{
						label: m.onboarding_machine_processor(),
						value: m.onboarding_machine_threads({ count: hardware.threads })
					}
				]
			: []
	);

	const model = $derived(MODEL_CHOICES.find((c) => c.variant === hardware?.recommended));

	const reason = $derived(
		hardware?.reason === 'gpu'
			? m.onboarding_machine_reason_gpu()
			: hardware?.reason === 'cpu'
				? m.onboarding_machine_reason_cpu()
				: m.onboarding_machine_reason_small()
	);
</script>

{#if !hardware}
	<p class="{hint} animate-pulse">{m.onboarding_machine_checking()}</p>
{:else}
	<dl class={group}>
		{#each rows as row (row.label)}
			<div class="flex items-baseline justify-between gap-6 px-4 py-3">
				<dt class="text-sm font-medium">{row.label}</dt>
				<dd class="text-right text-sm text-muted-foreground">{row.value}</dd>
			</div>
		{/each}
	</dl>

	{#if model}
		<div class="flex gap-3 rounded-lg border border-primary bg-primary/5 px-4 py-3">
			<SparklesIcon class="mt-0.5 size-4 shrink-0 text-primary" />
			<div class="flex flex-col gap-1">
				<p class="text-sm font-medium">
					{m.onboarding_machine_suggested({ model: model.name() })}
				</p>
				<p class="text-sm text-muted-foreground">{reason}</p>
			</div>
		</div>
	{/if}

	<p class={hint}>{m.onboarding_machine_estimate()}</p>
{/if}
