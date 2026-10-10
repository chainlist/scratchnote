<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import FilesIcon from '@lucide/svelte/icons/files';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import AtSignIcon from '@lucide/svelte/icons/at-sign';
	import MapIcon from '@lucide/svelte/icons/map';
	import PinOffIcon from '@lucide/svelte/icons/pin-off';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SunIcon from '@lucide/svelte/icons/sun';
	import { today, type Pin } from '#lib/api.js';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import * as ContextMenu from '#lib/components/ui/context-menu/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { coreIds } from '#lib/plugins/loader.js';
	import { monogram, pinHref, pinHue, pinPage } from '#lib/pins.js';
	import { errorText } from '#lib/errors.js';
	import { threadScope } from '#lib/threads.js';
	import { labelText, registry, type RibbonEntry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** Down the left edge of every view, a line between each group: the
	 *  days (today, All pages; the calendar opens from a day's date), what
	 *  ties the notes together (the names mentioned, SPEC 3.10, Threads with
	 *  how many are suggested, SPEC 6.4, the map, SPEC 6.5), the core
	 *  plugins' buttons, the community plugins' own (SPEC 3.9), then what
	 *  the space has pinned (SPEC 3.13). */
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
				const thread =
					shell.canSimilar && shell.threads.threadList.find((t) => t.id === pin.target);
				if (!thread) return [];
				const label = shell.threads.nameOf(thread);
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
		return shell.threads.threadList
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
	 *  day, a page or All pages, a name or Mentions, a thread or Threads,
	 *  the map. */
	const section = $derived.by(() => {
		if (atPin) return null;
		switch (page.route.id) {
			case '/(app)/day/[date]':
				return page.params.date === page.data.today ? 'today' : null;
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

	/** One of the edge's own buttons: the kind of view it shows, which marks
	 *  it while shown, and where it goes or what it does. */
	interface NavButton {
		kind: NonNullable<typeof section>;
		label: string;
		icon: Component;
		href?: string;
		onclick?: () => void;
		badge?: Snippet;
	}

	/** An edge button's look: in the accent colour while its kind of view is
	 *  shown. */
	const tone = (button: string) =>
		section === button
			? 'text-primary hover:text-primary'
			: 'text-muted-foreground hover:text-foreground';

	/** One that fails says so above the view. */
	function run(item: RibbonEntry) {
		const fail = (e: unknown) => shell.showError(`${item.plugin}: ${errorText(e)}`);
		try {
			void Promise.resolve(item.callback()).catch(fail);
		} catch (e) {
			fail(e);
		}
	}
</script>

<!-- Many pins in a short window scroll here, without a scrollbar, rather
     than take the whole window with them. -->
<nav
	aria-label={m.ribbon_label()}
	class="flex shrink-0 [scrollbar-width:none] flex-col items-center gap-1 overflow-y-auto px-2 py-3"
>
	{@render navButton({
		kind: 'today',
		label: m.command_today(),
		icon: SunIcon,
		onclick: async () => void shell.openDay(await today())
	})}
	{@render navButton({
		kind: 'pages',
		label: m.pages_all(),
		icon: FilesIcon,
		href: resolve('pages/')
	})}
	<Separator class="my-1 w-5!" />
	{@render navButton({
		kind: 'mentions',
		label: m.mentions_title(),
		icon: AtSignIcon,
		href: resolve('mentions/')
	})}
	{#if shell.canSimilar}
		{@render navButton({
			kind: 'threads',
			label: shell.threads.suggested
				? m.threads_with_suggested({ count: shell.threads.suggested })
				: m.threads_all(),
			icon: RouteIcon,
			href: resolve('threads/'),
			badge: suggestedCount
		})}
		{@render navButton({
			kind: 'map',
			label: m.map_title(),
			icon: MapIcon,
			href: resolve('map/')
		})}
	{/if}
	{#if core.length}
		<Separator class="my-1 w-5!" />
		{#each core as item (item)}{@render pluginButton(item)}{/each}
	{/if}
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
</nav>

<!-- One of the edge's own buttons, a view or a kind of view, marked while
     it is shown. -->
{#snippet navButton({ kind, label, icon: Icon, href, onclick, badge }: NavButton)}
	<Button
		variant="ghost"
		size="icon-sm"
		{href}
		{onclick}
		aria-label={label}
		title={label}
		class={['relative', tone(kind)]}
		aria-current={section === kind ? 'page' : undefined}
	>
		{@render marker(section === kind)}
		<Icon />{#if badge}
			{@render badge()}{/if}
	</Button>
{/snippet}

<!-- How many threads are suggested, on the Threads button. -->
{#snippet suggestedCount()}
	{#if shell.threads.suggested}
		<span
			class="absolute -top-0.5 -right-0.5 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-primary px-1 text-[0.6rem] leading-none font-semibold text-primary-foreground ring-2 ring-neutral-950"
		>
			{shell.threads.suggested > 9 ? '9+' : shell.threads.suggested}
		</span>
	{/if}
{/snippet}

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
	<ContextMenu.Root>
		<ContextMenu.Trigger>
			{#snippet child({ props })}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- pinHref resolves it -->
				<!-- The trigger's props take the link out of the tab order; it is
				     put back, and the Menu key or Shift+F10 opens its menu. -->
				<a
					{...props}
					tabindex={0}
					{href}
					aria-label={title}
					aria-current={active ? 'page' : undefined}
					{title}
					class="relative flex size-7 items-center justify-center rounded-md focus-ring outline-none"
				>
					{@render marker(active, false)}
					<span
						class={[
							'name-tint flex size-7 items-center justify-center rounded-md text-[0.7rem] leading-none font-semibold transition-opacity',
							active ? 'opacity-100' : 'opacity-85 hover:opacity-100'
						]}
						style:--hue={pinHue(pin)}
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
						<RouteIcon /><span class="truncate">{shell.threads.nameOf(thread)}</span>
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
