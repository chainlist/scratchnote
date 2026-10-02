<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import FilesIcon from '@lucide/svelte/icons/files';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { coreIds } from '#lib/plugins/loader.js';
	import { labelText, registry, type RibbonEntry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** Down the left edge of every view: the calendar, All pages and the
	 *  core plugins' buttons, then the community plugins' own (SPEC 3.9). */
	const shell = getShell();

	const core = $derived(registry.ribbon.filter((item) => coreIds.has(item.plugin)));
	const community = $derived(registry.ribbon.filter((item) => !coreIds.has(item.plugin)));

	/** One that fails says so above the view. */
	function run(item: RibbonEntry) {
		const fail = (e: unknown) =>
			shell.showError(`${item.plugin}: ${e instanceof Error ? e.message : String(e)}`);
		try {
			void Promise.resolve(item.callback()).catch(fail);
		} catch (e) {
			fail(e);
		}
	}
</script>

<div class="flex shrink-0 flex-col items-center gap-1 px-2 pt-3">
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('calendar/')}
		aria-label={m.calendar_pick()}
		title={m.calendar_pick()}
		class="text-muted-foreground hover:text-foreground"
	>
		<CalendarIcon />
	</Button>
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('pages/')}
		aria-label={m.pages_all()}
		title={m.pages_all()}
		class="text-muted-foreground hover:text-foreground"
	>
		<FilesIcon />
	</Button>
	{#each core as item (item)}{@render pluginButton(item)}{/each}
	{#if community.length}
		<Separator class="my-1 w-5!" />
		{#each community as item (item)}{@render pluginButton(item)}{/each}
	{/if}
</div>

{#snippet pluginButton(item: RibbonEntry)}
	{@const title = labelText(item.title)}
	<Button
		variant="ghost"
		size="icon-sm"
		onclick={() => run(item)}
		aria-label={title}
		{title}
		class="text-muted-foreground hover:text-foreground"
	>
		<PluginIcon icon={item.icon} />
	</Button>
{/snippet}
