<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';

	let {
		href,
		hue,
		class: className,
		children
	}: {
		/** Where it leads, as a link; left out, it is only a label. */
		href?: string;
		/** The name's colour, else the one its container sets. */
		hue?: number | string | null;
		/** Its size and room around it. */
		class?: ClassValue;
		/** The name as shown, `@` first. */
		children: Snippet;
	} = $props();
</script>

<!-- A name, as `@name`, tinted in its own colour. -->
{#if href}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- the caller resolves it -->
	<a {href} class={['name-tint rounded-md px-1 font-medium', className]} style:--hue={hue}
		>{@render children()}</a
	>
{:else}
	<span class={['name-tint rounded-md px-1 font-medium', className]} style:--hue={hue}
		>{@render children()}</span
	>
{/if}
