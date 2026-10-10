<script lang="ts">
	import type { StateCommand } from '@codemirror/state';
	import BoldIcon from '@lucide/svelte/icons/bold';
	import ItalicIcon from '@lucide/svelte/icons/italic';
	import LinkIcon from '@lucide/svelte/icons/link';
	import ListIcon from '@lucide/svelte/icons/list';
	import PaperclipIcon from '@lucide/svelte/icons/paperclip';
	import PluginIcon from '#lib/components/common/PluginIcon.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import { Toggle } from '#lib/components/ui/toggle/index.js';
	import { bold, bullets, italic, link } from '#lib/markdown/commands.js';
	import { m } from '#lib/paraglide/messages.js';
	import { labelText, registry, type ToolbarEntry } from '#lib/plugins/registry.svelte.js';

	let {
		active,
		pluginActive,
		onrun,
		onplugin,
		onattach,
		class: className = ''
	}: {
		/** The formats at the cursor, shown pressed. */
		active: { bold: boolean; italic: boolean; bullets: boolean };
		/** The plugins' buttons shown pressed, of those that can be. */
		pluginActive: ToolbarEntry[];
		/** Run a format on the text, which keeps the focus. */
		onrun: (command: StateCommand) => void;
		/** Run a plugin's button on the text. */
		onplugin: (button: ToolbarEntry) => void;
		/** Pick files to attach at the cursor, for the paperclip. */
		onattach: () => void;
		/** Such as a background to keep it in view on a long page. */
		class?: string;
	} = $props();

	// See-through greys, so the buttons show on every editor's background.
	const tool =
		'size-6 min-w-6 px-0 text-neutral-400 hover:bg-neutral-500/15 hover:text-neutral-200 aria-pressed:bg-neutral-500/25 aria-pressed:text-neutral-100 data-[state=on]:bg-neutral-500/25';
	const divider = 'mx-1 h-4 bg-neutral-500/30 data-vertical:self-center';
</script>

<!-- For those who do not write markdown (SPEC 3.4). A press keeps the
     focus, and so the selection, in the text; its buttons are what take
     the focus from the keyboard. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
	role="toolbar"
	aria-label={m.format_toolbar()}
	onmousedown={(event) => event.preventDefault()}
	class="mb-1 -ml-1 flex shrink-0 items-center gap-0.5 {className}"
>
	<Toggle
		size="sm"
		class={tool}
		bind:pressed={() => active.bold, () => onrun(bold)}
		aria-label={m.format_bold()}
		title={m.format_bold()}
	>
		<BoldIcon class="size-3.5" />
	</Toggle>
	<Toggle
		size="sm"
		class={tool}
		bind:pressed={() => active.italic, () => onrun(italic)}
		aria-label={m.format_italic()}
		title={m.format_italic()}
	>
		<ItalicIcon class="size-3.5" />
	</Toggle>
	{@render pluginTools('text')}
	<Separator orientation="vertical" class={divider} />
	<Toggle
		size="sm"
		class={tool}
		bind:pressed={() => active.bullets, () => onrun(bullets)}
		aria-label={m.format_bullets()}
		title={m.format_bullets()}
	>
		<ListIcon class="size-3.5" />
	</Toggle>
	{@render pluginTools('lists')}
	<Separator orientation="vertical" class={divider} />
	<Button
		variant="ghost"
		size="icon-xs"
		class={tool}
		onclick={() => onrun(link)}
		aria-label={m.format_link()}
		title={m.format_link()}
	>
		<LinkIcon class="size-3.5" />
	</Button>
	<Button
		variant="ghost"
		size="icon-xs"
		class={tool}
		onclick={onattach}
		aria-label={m.attach_file()}
		title={m.attach_file()}
	>
		<PaperclipIcon class="size-3.5" />
	</Button>
	{@render pluginTools('insert')}
</div>

<!-- The plugins' buttons in a group, such as the Tasks plugin's checklist
     beside the bulleted list (SPEC 3.9). -->
{#snippet pluginTools(group: 'text' | 'lists' | 'insert')}
	{#each registry.toolbar.filter((button) => (button.group ?? 'insert') === group) as button (button)}
		{@const title = labelText(button.title)}
		{#if button.active}
			<Toggle
				size="sm"
				class={tool}
				bind:pressed={() => pluginActive.includes(button), () => onplugin(button)}
				aria-label={title}
				{title}
			>
				<PluginIcon icon={button.icon} class="size-3.5" />
			</Toggle>
		{:else}
			<Button
				variant="ghost"
				size="icon-xs"
				class={tool}
				onclick={() => onplugin(button)}
				aria-label={title}
				{title}
			>
				<PluginIcon icon={button.icon} class="size-3.5" />
			</Button>
		{/if}
	{/each}
{/snippet}
