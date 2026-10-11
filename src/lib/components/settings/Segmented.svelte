<script lang="ts" generics="T extends string | number">
	import { RadioGroup } from 'bits-ui';
	import { follow } from '#lib/helpers/motion.js';

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
     Tab, the arrows move between the options, as a set of radios does. The
     fill glides to the option picked. -->
<RadioGroup.Root
	value={String(value)}
	onValueChange={(picked) => {
		const option = options.find((o) => String(o.value) === picked);
		if (option) onpick(option.value);
	}}
	orientation="horizontal"
	aria-label={label}
	class="relative flex gap-0.5 rounded-lg border p-0.5"
>
	<span
		aria-hidden="true"
		class="glide-mark rounded-md bg-primary"
		{@attach follow('[data-state="checked"]')}
	></span>
	{#each options as option (option.value)}
		<RadioGroup.Item
			value={String(option.value)}
			class="relative h-7 rounded-md px-3 text-xs font-medium whitespace-nowrap focus-ring transition-colors duration-250 ease-settle outline-none {value ===
			option.value
				? 'text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-foreground'}"
		>
			{option.label}
		</RadioGroup.Item>
	{/each}
</RadioGroup.Root>
