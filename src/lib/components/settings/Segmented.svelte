<script lang="ts" generics="T extends string | number">
	let {
		label,
		options,
		value,
		onpick
	}: {
		label: string;
		options: { value: T; label: string }[];
		value: T;
		onpick: (value: T) => void;
	} = $props();
</script>

<!-- A segmented picker: one option filled, the others plain. -->
<div class="flex gap-0.5 rounded-lg border p-0.5" role="radiogroup" aria-label={label}>
	{#each options as option (option.value)}
		<button
			type="button"
			role="radio"
			aria-checked={value === option.value}
			onclick={() => onpick(option.value)}
			class="h-7 rounded-md px-3 text-xs font-medium whitespace-nowrap transition-colors {value ===
			option.value
				? 'bg-primary text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-foreground'}"
		>
			{option.label}
		</button>
	{/each}
</div>
