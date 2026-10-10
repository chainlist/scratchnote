<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import Ribbon from '#lib/components/layout/Ribbon.svelte';
	import { closeOnBack } from '#lib/back.svelte.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { inText } from '#lib/helpers/dom.js';
	import { onSwipe } from '#lib/helpers/swipe.js';

	/** The ribbon on a phone: off screen until the menu button or a swipe in
	 *  from the left edge draws it out over the view. A swipe back, a tap
	 *  beside it, Back or going to another view puts it away. */
	const shell = getShell();

	/** A touch starting this close to the left edge draws the drawer out. On
	 *  Android only a band of the edge reaches the app (MainActivity.kt); the
	 *  rest is the system's Back. */
	const EDGE = 24;

	let width = $state(0);
	/** How far out the drawer is while a finger moves it, in px. */
	let dragged = $state<number | null>(null);

	const clamp = (x: number) => Math.min(width, Math.max(0, x));
	const shown = $derived(dragged ?? (shell.ribbonOpen ? width : 0));
	const progress = $derived(width ? shown / width : 0);

	closeOnBack(
		() => shell.ribbonOpen,
		() => (shell.ribbonOpen = false)
	);
	afterNavigate(() => (shell.ribbonOpen = false));

	$effect(() =>
		onSwipe(document, {
			axis: 'x',
			accept: (touch, target) =>
				shell.ribbonOpen ||
				(touch.clientX < EDGE && !inText(target) && !target.closest('[role="dialog"]')),
			move: (x) => (dragged = clamp(shell.ribbonOpen ? width + x : x)),
			end: (x, speed) => {
				dragged = null;
				shell.ribbonOpen = shell.ribbonOpen
					? x > -width / 2 && speed > -0.3
					: x > width / 2 || speed > 0.3;
			}
		})
	);
</script>

<div class="pointer-events-none fixed inset-0 z-40">
	<button
		type="button"
		tabindex="-1"
		aria-label={m.common_close()}
		class={['absolute inset-0 bg-black/60', progress > 0 && 'pointer-events-auto']}
		class:transition-opacity={dragged === null}
		style:opacity={progress}
		onclick={() => (shell.ribbonOpen = false)}
	></button>
	<!-- A button that does not lead to another view, as a plugin's, puts it
	     away too. -->
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div
		bind:offsetWidth={width}
		inert={!shell.ribbonOpen}
		class="pointer-events-auto absolute inset-y-0 left-0 flex flex-col border-r bg-background"
		class:transition-transform={dragged === null}
		style:transform="translateX(calc({shown}px - 100%))"
		onclick={(event) => {
			if ((event.target as Element).closest('a, button')) shell.ribbonOpen = false;
		}}
	>
		<Ribbon />
	</div>
</div>
