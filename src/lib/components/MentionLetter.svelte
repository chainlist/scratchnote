<script lang="ts">
	import type { ClassValue, HTMLAttributes } from 'svelte/elements';

	let {
		name,
		large = false,
		hue,
		class: className,
		...rest
	}: Omit<HTMLAttributes<HTMLSpanElement>, 'class'> & {
		name: string;
		/** As a name's own view heads it, rather than as the list of names shows it. */
		large?: boolean;
		/** The name's colour, else the one its container sets. */
		hue?: number;
		class?: ClassValue;
	} = $props();
</script>

<!-- A name's first letter in its own colour, as its tile. -->
<span
	{...rest}
	class={[
		'name-tint flex shrink-0 items-center justify-center font-semibold',
		large ? 'size-11 rounded-xl text-lg' : 'size-8 rounded-lg text-sm',
		className
	]}
	style:--hue={hue}
>
	{[...name][0]?.toUpperCase()}
</span>
