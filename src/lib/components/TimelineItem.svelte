<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue, HTMLAttributes } from 'svelte/elements';

	/**
	 * One item of a timeline, as the day draws its notes: the time on the
	 * left, the rail with the item's mark on it, and the body on the right.
	 * Every timeline of the app is made of these, the plugins' too (through
	 * `Timeline` in the plugin API), so they all look alike. Each sits in an
	 * `li` of the list, which is how the first and last know to stop the rail
	 * at their mark.
	 */
	let {
		time,
		date,
		ontime,
		timeTitle,
		icon,
		hover = true,
		class: className,
		children,
		...rest
	}: HTMLAttributes<HTMLElement> & {
		time: string;
		/** Above the time, for a timeline that spans days. */
		date?: string;
		/** Makes the time a button, such as to the note on its day. */
		ontime?: () => void;
		timeTitle?: string;
		/** A mark in a box on the rail, as a page has, in place of the dot. */
		icon?: Snippet;
		/** A background under the item while the pointer or the focus is in it. */
		hover?: boolean;
		class?: ClassValue;
		/** The body's column, and whatever is laid over the item. */
		children: Snippet;
	} = $props();

	const when = 'self-start pt-1 text-right font-mono text-xs leading-5 text-neutral-600';
</script>

<article
	{...rest}
	class={[
		'group relative isolate -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 rounded-lg px-3 py-5',
		hover &&
			'transition-colors duration-300 ease-out focus-within:bg-neutral-900 hover:bg-neutral-900',
		className
	]}
>
	<!-- The rail in the gutter between time and body. Each item draws its own
	     mark and the segments above and below it; the first and last items
	     leave off the outer ends so the rail stops at their marks. -->
	{#if icon}
		<span
			aria-hidden="true"
			class="absolute top-0 left-25 h-6.5 w-px bg-neutral-800 [li:first-child_&]:hidden"
		></span>
		<span
			aria-hidden="true"
			class="absolute top-6.5 left-25 flex size-4.5 translate-x-[-8.5px] items-center justify-center rounded border border-neutral-700 bg-neutral-950 text-neutral-500 transition-colors duration-300 group-hover:border-neutral-500 group-hover:text-neutral-300"
		>
			{@render icon()}
		</span>
		<span
			aria-hidden="true"
			class="absolute top-[44px] bottom-0 left-25 w-px bg-neutral-800 [li:last-child_&]:hidden"
		></span>
	{:else}
		<span
			aria-hidden="true"
			class="absolute top-0 left-25 h-[30px] w-px bg-neutral-800 [li:first-child_&]:hidden"
		></span>
		<span
			aria-hidden="true"
			class="absolute top-[30px] left-25 size-2 -translate-x-[3.5px] rounded-full border border-neutral-700 bg-neutral-950 transition-colors duration-300 group-hover:border-neutral-400 group-hover:bg-neutral-400"
		></span>
		<span
			aria-hidden="true"
			class="absolute top-[38px] bottom-0 left-25 w-px bg-neutral-800 [li:last-child_&]:hidden"
		></span>
	{/if}
	{#if ontime}
		<button
			type="button"
			onclick={ontime}
			title={timeTitle}
			class="{when} cursor-pointer transition-colors hover:text-neutral-300"
		>
			{#if date}<span class="block">{date}</span>{/if}{time}
		</button>
	{:else}
		<time class={when}>
			{#if date}<span class="block">{date}</span>{/if}{time}
		</time>
	{/if}
	{@render children()}
</article>
