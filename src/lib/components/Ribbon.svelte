<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import CalendarCheckIcon from '@lucide/svelte/icons/calendar-check';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import FilesIcon from '@lucide/svelte/icons/files';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import AtSignIcon from '@lucide/svelte/icons/at-sign';
	import MapIcon from '@lucide/svelte/icons/map';
	import PinOffIcon from '@lucide/svelte/icons/pin-off';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { today, type Pin } from '#lib/api.js';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import * as ContextMenu from '#lib/components/ui/context-menu/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { coreIds } from '#lib/plugins/loader.js';
	import { colourOf, monogram, pinHref, pinPage } from '#lib/pins.js';
	import { labelText, registry, type RibbonEntry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** Down the left edge of every view: today, the calendar, All pages, the
	 *  names mentioned (SPEC 3.10), Threads
	 *  with how many are suggested (SPEC 6.4), the map (SPEC 6.5) and the
	 *  core plugins' buttons, then the community plugins' own (SPEC 3.9),
	 *  then what the space has pinned (SPEC 3.13). */
	const shell = getShell();

	const core = $derived(registry.ribbon.filter((item) => coreIds.has(item.plugin)));
	const community = $derived(registry.ribbon.filter((item) => !coreIds.has(item.plugin)));

	/** The pins that lead somewhere, with their titles: a thread still there
	 *  while threads are on offer, a page whose plugin is on. */
	const pins = $derived(
		shell.pins.flatMap((pin) => {
			if (pin.kind === 'thread') {
				const thread = shell.canSimilar && shell.threadList.find((t) => t.id === pin.target);
				return thread ? [{ pin, label: thread.title ?? m.thread_untitled() }] : [];
			}
			if (pin.kind === 'mention') return [{ pin, label: `@${pin.label ?? pin.target}` }];
			const type = pinPage(pin);
			const shown = registry.pages.some((entry) => entry.type === type);
			return shown ? [{ pin, label: pin.label ?? type }] : [];
		})
	);

	const shownPins = $derived(pins.map(({ pin }) => pin));

	const here = $derived(`${page.url.pathname}${page.url.search}`);

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
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('mentions/')}
		aria-label={m.mentions_title()}
		title={m.mentions_title()}
		class="text-muted-foreground hover:text-foreground"
	>
		<AtSignIcon />
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
	{#if pins.length}
		<Separator class="my-1 w-5!" />
		{#each pins as { pin, label }, index (`${pin.kind} ${pin.target}`)}
			{@render pinButton(pin, label, index)}
		{/each}
	{/if}
</div>

{#snippet pinButton(pin: Pin, label: string, index: number)}
	{@const href = pinHref(pin)}
	{@const active = here === href}
	{@const colour = colourOf(pin.target)}
	<ContextMenu.Root>
		<ContextMenu.Trigger>
			{#snippet child({ props })}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- pinHref resolves it -->
				<a
					{...props}
					{href}
					aria-label={label}
					aria-current={active ? 'page' : undefined}
					title={label}
					class="relative flex size-8 items-center justify-center rounded-md outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
				>
					{#if active}
						<span class="absolute top-1.5 -left-2 h-5 w-1 rounded-r bg-foreground"></span>
					{/if}
					<span
						class={[
							'flex size-7 items-center justify-center rounded-md text-[0.7rem] leading-none font-semibold transition-opacity',
							active ? 'opacity-100' : 'opacity-75 hover:opacity-100'
						]}
						style:color={colour}
						style:background-color="color-mix(in oklab, {colour} 18%, transparent)"
					>
						{monogram(label)}
					</span>
				</a>
			{/snippet}
		</ContextMenu.Trigger>
		<ContextMenu.Content class="w-48">
			<ContextMenu.Label class="truncate">{label}</ContextMenu.Label>
			<ContextMenu.Separator />
			<ContextMenu.Item
				disabled={index === 0}
				onSelect={() => void shell.movePin(pin, -1, shownPins)}
			>
				<ArrowUpIcon />{m.pin_move_up()}
			</ContextMenu.Item>
			<ContextMenu.Item
				disabled={index === pins.length - 1}
				onSelect={() => void shell.movePin(pin, 1, shownPins)}
			>
				<ArrowDownIcon />{m.pin_move_down()}
			</ContextMenu.Item>
			<ContextMenu.Item onSelect={() => void shell.togglePin(pin)}>
				<PinOffIcon />{m.pin_remove()}
			</ContextMenu.Item>
		</ContextMenu.Content>
	</ContextMenu.Root>
{/snippet}

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
