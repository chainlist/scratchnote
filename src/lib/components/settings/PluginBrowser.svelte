<script lang="ts">
	import { onMount } from 'svelte';
	import {
		browsePlugins,
		installPlugin,
		openLink,
		pluginDetails,
		type PluginDetails,
		type RegistryEntry
	} from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Spinner } from '#lib/components/ui/spinner/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { hasEvery, queryWords } from '#lib/matching.js';
	import { m } from '#lib/paraglide/messages.js';
	import { adopt, coreIds, plugins, setCommunityPlugin } from '#lib/plugins/loader.js';
	import { older } from '#lib/versions.js';
	import type { SettingsState } from './state.svelte';
	import { hint } from './styles';

	/**
	 * The community plugins the registry lists (SPEC 4.10), searched by name,
	 * author and description, and one plugin's details: its README, and a
	 * button to install it, update it, or switch it on.
	 */
	let { settings, onback }: { settings: SettingsState; onback: () => void } = $props();

	let entries = $state<RegistryEntry[] | null>(null);
	let failed = $state<string | null>(null);
	let query = $state('');
	let chosen = $state<RegistryEntry | null>(null);
	let details = $state<PluginDetails | null>(null);
	let detailsFailed = $state<string | null>(null);
	let installing = $state(false);

	async function load() {
		entries = null;
		failed = null;
		try {
			entries = await browsePlugins();
		} catch (e) {
			failed = String(e);
		}
	}

	onMount(() => void load());

	const shown = $derived.by(() => {
		const words = queryWords(query);
		return (entries ?? []).filter((entry) =>
			hasEvery(`${entry.name} ${entry.author} ${entry.description}`, words)
		);
	});

	const installedOf = (id: string) => plugins.view.installed.find((manifest) => manifest.id === id);

	async function choose(entry: RegistryEntry) {
		chosen = entry;
		details = null;
		detailsFailed = null;
		try {
			const found = await pluginDetails(entry.repo);
			if (chosen === entry) details = found;
		} catch (e) {
			if (chosen === entry) detailsFailed = String(e);
		}
	}

	async function install(entry: RegistryEntry) {
		installing = true;
		try {
			await adopt(await installPlugin(entry.repo, entry.id));
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			installing = false;
		}
	}

	function back() {
		if (chosen) chosen = null;
		else onback();
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center gap-2">
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={back}
			aria-label={m.plugins_back()}
			title={m.plugins_back()}
			class="text-muted-foreground hover:text-foreground"
		>
			<ArrowLeftIcon />
		</Button>
		<h4 class="min-w-0 flex-1 truncate text-sm font-semibold">
			{chosen ? chosen.name : m.plugins_group_community()}
		</h4>
		{#if !chosen}
			<Input
				bind:value={query}
				placeholder={m.plugins_search()}
				aria-label={m.plugins_search()}
				class="h-7 w-56"
			/>
		{/if}
	</div>

	{#if chosen}
		{@const entry = chosen}
		{@const installed = installedOf(entry.id)}
		{@const latest = details?.manifest}
		{@const clash = coreIds.has(entry.id)}
		<div class="flex flex-col gap-4">
			<div class="flex items-start justify-between gap-6">
				<div class="flex min-w-0 flex-col gap-1">
					<p class={hint}>{m.plugins_by({ author: entry.author })}</p>
					<p class="text-sm">{entry.description}</p>
					<div class="flex flex-wrap items-center gap-2 pt-1">
						{#if latest}
							<Badge variant="secondary" class="font-mono">{latest.version}</Badge>
						{/if}
						<Button
							variant="link"
							size="sm"
							class="h-auto px-0 text-xs text-muted-foreground"
							onclick={() => openLink(`https://github.com/${entry.repo}`)}
						>
							{entry.repo}<ExternalLinkIcon />
						</Button>
					</div>
				</div>
				<div class="flex shrink-0 items-center gap-2">
					{#if clash}
						<Badge variant="destructive">{m.plugins_error_core_id()}</Badge>
					{:else if !installed}
						<Button size="sm" onclick={() => install(entry)} disabled={installing || !latest}>
							<DownloadIcon class={installing ? 'animate-pulse' : ''} />
							{installing ? m.plugins_installing() : m.plugins_install()}
						</Button>
					{:else}
						{#if latest && older(installed.version, latest.version)}
							<Button size="sm" onclick={() => install(entry)} disabled={installing}>
								<DownloadIcon class={installing ? 'animate-pulse' : ''} />
								{m.plugins_update_to({ version: latest.version })}
							</Button>
						{/if}
						<label class="flex items-center gap-2 text-sm">
							{m.plugins_enable()}
							<Switch
								bind:checked={
									() => plugins.view.enabled.includes(entry.id),
									(on) =>
										void setCommunityPlugin(entry.id, on).catch((e) =>
											settings.say(String(e), true)
										)
								}
							/>
						</label>
					{/if}
				</div>
			</div>

			<div class="rounded-lg border bg-card px-4 py-3">
				{#if detailsFailed}
					<p class="text-sm text-destructive">{detailsFailed}</p>
				{:else if !details}
					<p class="flex items-center gap-2 {hint}">
						<Spinner class="size-3.5" aria-hidden="true" />{m.plugins_loading()}
					</p>
				{:else if details.readme}
					<Markdown text={details.readme} class="text-sm leading-6" />
				{:else}
					<p class={hint}>{m.plugins_readme_none()}</p>
				{/if}
			</div>
		</div>
	{:else if failed}
		<div class="flex flex-col items-start gap-3">
			<p class="text-sm text-destructive">{failed}</p>
			<Button variant="secondary" size="sm" onclick={load}>
				<RefreshCwIcon />{m.plugins_retry()}
			</Button>
		</div>
	{:else if entries === null}
		<p class="flex items-center gap-2 {hint}">
			<Spinner class="size-3.5" aria-hidden="true" />{m.plugins_loading()}
		</p>
	{:else if shown.length === 0}
		<p class={hint}>{m.plugins_browse_empty()}</p>
	{:else}
		<ul class="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-3">
			{#each shown as entry (entry.id)}
				<li class="flex">
					<!-- The name's button stretches over the whole card. -->
					<Card.Root
						size="sm"
						class="relative flex-1 transition-colors hover:bg-muted/50 has-focus-visible:ring-2 has-focus-visible:ring-ring"
					>
						<Card.Header>
							<Card.Title class="min-w-0">
								<button
									type="button"
									onclick={() => choose(entry)}
									class="block max-w-full cursor-pointer truncate text-left after:absolute after:inset-0 focus-visible:outline-none"
								>
									{entry.name}
								</button>
							</Card.Title>
							<Card.Description class={hint}>
								{m.plugins_by({ author: entry.author })}
							</Card.Description>
							{#if installedOf(entry.id)}
								<Card.Action>
									<Badge variant="secondary">{m.plugins_installed_badge()}</Badge>
								</Card.Action>
							{/if}
						</Card.Header>
						<Card.Content>
							<p class="line-clamp-3 text-xs text-muted-foreground">{entry.description}</p>
						</Card.Content>
					</Card.Root>
				</li>
			{/each}
		</ul>
	{/if}
</div>
