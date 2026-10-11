<script lang="ts">
	import { tick } from 'svelte';
	import type { Note } from '#lib/api.js';
	import NoteActionItems from '#lib/components/note/NoteActionItems.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import { m } from '#lib/paraglide/messages.js';

	let {
		note,
		when,
		armed = $bindable(false),
		onedit,
		onsimilar,
		onpage,
		onmove,
		ondelete
	}: {
		note: Note;
		/** The note's time, and its day where the list spans days. */
		when: string;
		/** The menu is drawn once the card is pointed at or focused, as its
		 *  button only shows then: a menu on every card is most of what a long
		 *  list takes to draw. Until then, a plain button stands in for its trigger. */
		armed?: boolean;
		/** Edit the note, once the menu has closed. */
		onedit: () => void;
		onsimilar?: (note: Note) => void;
		onpage: (note: Note) => void;
		onmove?: (note: Note) => void;
		ondelete: (note: Note) => void;
	} = $props();

	let open = $state(false);
	let standIn = $state<HTMLButtonElement | null>(null);
	let trigger = $state<HTMLElement | null>(null);
	/** Edit from the menu waits for it to close, or it takes the focus back. */
	let editAfterMenu = false;

	const menuButton =
		'flex size-6 cursor-pointer items-center justify-center rounded text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200';

	/** Draw the menu; focus on the stand-in moves to the trigger replacing it. */
	export async function arm() {
		if (armed) return;
		const focused = standIn !== null && document.activeElement === standIn;
		armed = true;
		if (focused) {
			await tick();
			trigger?.focus();
		}
	}

	export function focus() {
		(trigger ?? standIn)?.focus();
	}
</script>

<!-- The note's menu in the margin, beside the time it acts on; over the
     text's right end when the time heads the text, in a narrow container. -->
<div
	class="absolute top-3.5 left-1.5 flex transition duration-200 ease-settle group-hover:translate-x-0 group-hover:opacity-100 focus-within:translate-x-0 focus-within:opacity-100 motion-reduce:translate-x-0 @max-[24rem]:top-2.5 @max-[24rem]:right-3 @max-[24rem]:left-auto {open
		? 'opacity-100'
		: 'translate-x-1 opacity-0'}"
>
	{#if !armed}
		<button
			bind:this={standIn}
			type="button"
			aria-label={m.note_actions_at({ time: when })}
			title={m.note_actions()}
			aria-haspopup="menu"
			aria-expanded="false"
			onclick={() => {
				open = true;
				void arm();
			}}
			class={menuButton}
		>
			<EllipsisIcon class="size-3.5" />
		</button>
	{:else}
		<DropdownMenu.Root bind:open>
			<DropdownMenu.Trigger
				bind:ref={trigger}
				aria-label={m.note_actions_at({ time: when })}
				title={m.note_actions()}
				class={menuButton}
			>
				<EllipsisIcon class="size-3.5" />
			</DropdownMenu.Trigger>
			<DropdownMenu.Content
				align="start"
				collisionPadding={8}
				class="w-max max-w-80 min-w-52"
				onCloseAutoFocus={(event) => {
					if (!editAfterMenu) return;
					editAfterMenu = false;
					event.preventDefault();
					onedit();
				}}
			>
				<NoteActionItems
					{note}
					onedit={() => (editAfterMenu = true)}
					{onsimilar}
					{onpage}
					{onmove}
					{ondelete}
				/>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/if}
</div>
