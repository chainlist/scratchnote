<script lang="ts" generics="T extends string | number">
	import { RadioGroup } from 'bits-ui';

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

<!-- A segmented picker: one option filled, the others plain. One stop for
     Tab, the arrows move between the options, as a set of radios does. -->
<RadioGroup.Root
	value={String(value)}
	onValueChange={(picked) => {
		const option = options.find((o) => String(o.value) === picked);
		if (option) onpick(option.value);
	}}
	orientation="horizontal"
	aria-label={label}
	class="flex gap-0.5 rounded-lg border p-0.5"
>
	{#each options as option (option.value)}
		<RadioGroup.Item
			value={String(option.value)}
			class="h-7 rounded-md px-3 text-xs font-medium whitespace-nowrap transition-colors outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring focus-visible:outline-solid {value ===
			option.value
				? 'bg-primary text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-foreground'}"
		>
			{option.label}
		</RadioGroup.Item>
	{/each}
</RadioGroup.Root>
