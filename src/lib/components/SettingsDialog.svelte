<script lang="ts">
	import Settings from '#lib/components/settings/Settings.svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { inText } from '#lib/dom.js';
	import { onSwipe, phone } from '#lib/swipe.js';

	let { open = $bindable(false) }: { open?: boolean } = $props();

	let dialog = $state<HTMLElement | null>(null);

	// On a phone the settings take the whole screen, and pulling them down
	// from their top closes them, as their X does.
	$effect(() => {
		const box = dialog;
		if (!box || !phone.current) return;
		const slide = (y: number, animate: boolean) => {
			box.style.transition = animate ? 'transform 150ms ease-out' : '';
			box.style.transform = y ? `translateY(${y}px)` : '';
		};
		return onSwipe(box, {
			axis: 'y',
			// Only from the top: a list scrolled down scrolls back up first.
			accept: (_, target) => {
				if (inText(target)) return false;
				for (let el: Element | null = target; el && el !== box; el = el.parentElement)
					if (el.scrollTop > 0) return false;
				return true;
			},
			move: (y) => slide(Math.max(0, y), false),
			end: (y, speed) => {
				if (y > 120 || (y > 30 && speed > 0.5)) open = false;
				else slide(0, true);
			}
		});
	});
</script>

<!-- The whole screen on a phone. The focus goes to the dialog itself rather
     than the first tab, which opened by Ctrl+, would show its focus ring; Tab
     goes on to the tabs. -->
<Dialog.Root bind:open>
	<Dialog.Content
		bind:ref={dialog}
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			dialog?.focus();
		}}
		class="h-dvh max-w-full grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden rounded-none p-0 ring-0 sm:h-[min(1000px,85vh)] sm:max-w-[min(1100px,calc(100%-2rem))] sm:rounded-xl sm:ring-1"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
