<script lang="ts">
	import { iconSvg } from '$lib/plugins/icons';

	/** A plugin's icon: its SVG markup, or the puzzle piece without one. */
	let { icon, class: className = 'size-4' }: { icon?: string; class?: string } = $props();
</script>

<!-- Plugins are trusted code (SPEC 3.9), and their icons with them. -->
<!-- eslint-disable-next-line svelte/no-at-html-tags -->
<span class="inline-flex shrink-0 {className} [&>svg]:size-full">{@html iconSvg(icon)}</span>
