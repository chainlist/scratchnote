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
	import { threadScope } from '#lib/threads.js';
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

	/** The pins that lead somewhere, each with what its letters are made
	 *  from and its title: a thread still there while threads are on offer,
	 *  its name before a name's thread's title, a name, a page whose plugin
	 *  is on. */
	const pins = $derived(
		shell.pins.flatMap((pin) => {
			if (pin.kind === 'thread') {
				const thread = shell.canSimilar && shell.threadList.find((t) => t.id === pin.target);
				if (!thread) return [];
				const label = thread.title ?? m.thread_untitled();
				const title = thread.mention ? `@${thread.mention} · ${label}` : label;
				return [{ pin, label, title }];
			}
			if (pin.kind === 'mention') {
				const label = `@${pin.label ?? pin.target}`;
				return [{ pin, label, title: label }];
			}
			const type = pinPage(pin);
			const shown = registry.pages.some((entry) => entry.type === type);
			const label = pin.label ?? type;
			return shown ? [{ pin, label, title: label }] : [];
		})
	);

	/** A pinned name's threads of the user's, the one written in last first,
	 *  one level below it (SPEC 3.13). */
	function threadsOf(pin: Pin) {
		if (pin.kind !== 'mention' || !shell.canSimilar) return [];
		return shell.threadList
			.filter((thread) => thread.kept && thread.scope === pin.target)
			.toSorted((a, b) => b.until.localeCompare(a.until))
			.slice(0, 8);
	}

	/** The view shown is the pin's, or for a name, one of its threads. */
	function isHere(pin: Pin, href: string) {
		if (here === href) return true;
		return (
			pin.kind === 'mention' &&
			page.route.id === '/(app)/thread/[id]' &&
			threadScope(page.params.id ?? '') === pin.target
		);
	}

	const shownPins = $derived(pins.map(({ pin }) => pin));

	const here = $derived(`${page.url.pathname}${page.url.search}`);

	/** A pin's view is shown: the pin marks where the user is, rather than
	 *  the button of that kind of view. */
	const atPin = $derived(pins.some(({ pin }) => isHere(pin, pinHref(pin))));

	/** The edge's own button whose kind of view is shown, if any: today's
	 *  day, the calendar, a page or All pages, a name or Mentions, a thread
	 *  or Threads, the map. */
	const section = $derived.by(() => {
		if (atPin) return null;
		switch (page.route.id) {
			case '/(app)/day/[date]':
				return page.params.date === page.data.today ? 'today' : null;
			case '/(app)/calendar':
				return 'calendar';
			case '/(app)/pages':
			case '/(app)/page/[date]/[id]':
				return 'pages';
			case '/(app)/mentions':
			case '/(app)/mention/[name]':
				return 'mentions';
			case '/(app)/threads':
			case '/(app)/thread/[id]':
				return 'threads';
			case '/(app)/map':
				return 'map';
			default:
				return null;
		}
	});

	/** An edge button's look: in the accent colour while its kind of view is
	 *  shown. */
	const tone = (button: string) =>
		section === button
			? 'text-primary hover:text-primary'
			: 'text-muted-foreground hover:text-foreground';

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
		class={['relative', tone('today')]}
		aria-current={section === 'today' ? 'page' : undefined}
	>
		{@render marker(section === 'today')}
		<CalendarCheckIcon />
	</Button>
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('calendar/')}
		aria-label={m.calendar_pick()}
		title={m.calendar_pick()}
		class={['relative', tone('calendar')]}
		aria-current={section === 'calendar' ? 'page' : undefined}
	>
		{@render marker(section === 'calendar')}
		<CalendarIcon />
	</Button>
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('pages/')}
		aria-label={m.pages_all()}
		title={m.pages_all()}
		class={['relative', tone('pages')]}
		aria-current={section === 'pages' ? 'page' : undefined}
	>
		{@render marker(section === 'pages')}
		<FilesIcon />
	</Button>
	<Button
		variant="ghost"
		size="icon-sm"
		href={resolve('mentions/')}
		aria-label={m.mentions_title()}
		title={m.mentions_title()}
		class={['relative', tone('mentions')]}
		aria-current={section === 'mentions' ? 'page' : undefined}
	>
		{@render marker(section === 'mentions')}
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
			class={['relative', tone('threads')]}
			aria-current={section === 'threads' ? 'page' : undefined}
		>
			{@render marker(section === 'threads')}
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
			class={['relative', tone('map')]}
			aria-current={section === 'map' ? 'page' : undefined}
		>
			{@render marker(section === 'map')}
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
		{#each pins as { pin, label, title }, index (`${pin.kind} ${pin.target}`)}
			{@render pinButton(pin, label, title, index)}
		{/each}
	{/if}
</div>

<!-- The bar beside a button whose view is shown, at the rail's edge. A
     Button's border is left out of where it is placed from, so the bar
     steps back over it to line up with a pin's. -->
{#snippet marker(active: boolean, bordered = true)}
	{#if active}
		<span class={['absolute top-1 -left-2 h-5 w-1 rounded-r bg-primary', bordered && '-m-px']}
		></span>
	{/if}
{/snippet}

{#snippet pinButton(pin: Pin, label: string, title: string, index: number)}
	{@const href = pinHref(pin)}
	{@const active = isHere(pin, href)}
	{@const below = threadsOf(pin)}
	{@const colour = colourOf(pin.target)}
	<ContextMenu.Root>
		<ContextMenu.Trigger>
			{#snippet child({ props })}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- pinHref resolves it -->
				<a
					{...props}
					{href}
					aria-label={title}
					aria-current={active ? 'page' : undefined}
					{title}
					class="relative flex size-7 items-center justify-center rounded-md outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
				>
					{@render marker(active, false)}
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
		<ContextMenu.Content class="w-56">
			<ContextMenu.Label class="truncate">{title}</ContextMenu.Label>
			<ContextMenu.Separator />
			{#if below.length}
				{#each below as thread (thread.id)}
					<ContextMenu.Item onSelect={() => void shell.showThread(thread.id)}>
						<RouteIcon /><span class="truncate">{thread.title ?? m.thread_untitled()}</span>
					</ContextMenu.Item>
				{/each}
				<ContextMenu.Separator />
			{/if}
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
