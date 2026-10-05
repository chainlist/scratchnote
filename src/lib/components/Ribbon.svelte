<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import CalendarCheckIcon from '@lucide/svelte/icons/calendar-check';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import FilesIcon from '@lucide/svelte/icons/files';
	import MapIcon from '@lucide/svelte/icons/map';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { today } from '#lib/api.js';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { coreIds } from '#lib/plugins/loader.js';
	import { labelText, registry, type RibbonEntry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** Down the left edge of every view: today, the calendar, All pages, Threads
	 *  with how many are suggested (SPEC 6.4), the map (SPEC 6.5) and the
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
		onclick={async () => void shell.openDay(await today())}
		aria-label={m.command_today()}
		title={m.command_today()}
		class="text-muted-foreground hover:text-foreground"
	>
		<CalendarCheckIcon />
	</Button>
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
	{#if shell.canSimilar}
		{@const title = shell.suggested
			? m.threads_with_suggested({ count: shell.suggested })
			: m.threads_all()}
		<Button
			variant="ghost"
			size="icon-sm"
			href={resolve('threads/')}
			aria-label={title}
			{title}
			class="relative text-muted-foreground hover:text-foreground"
		>
			<RouteIcon />
			{#if shell.suggested}
				<span
					class="absolute -top-0.5 -right-0.5 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-primary px-1 text-[0.6rem] leading-none font-medium text-primary-foreground"
				>
					{shell.suggested > 9 ? '9+' : shell.suggested}
				</span>
			{/if}
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			href={resolve('map/')}
			aria-label={m.map_title()}
			title={m.map_title()}
			class="text-muted-foreground hover:text-foreground"
		>
			<MapIcon />
		</Button>
	{/if}
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
