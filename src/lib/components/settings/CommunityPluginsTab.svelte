<script lang="ts">
	import {
		browsePlugins,
		installPlugin,
		pluginDetails,
		uninstallPlugin,
		type PluginManifest
	} from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import ShieldAlertIcon from '@lucide/svelte/icons/shield-alert';
	import StoreIcon from '@lucide/svelte/icons/store';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { m } from '#lib/paraglide/messages.js';
	import { adopt, older, plugins, setCommunity, setCommunityPlugin } from '#lib/plugins/loader.js';
	import PluginBrowser from './PluginBrowser.svelte';
	import PluginOptions from './PluginOptions.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint, section } from './styles';

	/**
	 * Settings > Community plugins (SPEC 3.9): off as a whole until the
	 * user turns them on, trusting their authors, then browsed, installed,
	 * updated and removed.
	 */
	let { settings }: { settings: SettingsState } = $props();

	let browsing = $state(false);
	/** The trust warning, shown before community plugins turn on. */
	let trusting = $state(false);
	/** The plugin waiting on the uninstall confirmation. */
	let removing = $state<PluginManifest | null>(null);
	/** Newer versions a check found, by id, and the repository they come from. */
	let updates = $state<Record<string, { version: string; repo: string }>>({});
	let checking = $state(false);
	/** The plugin being updated or uninstalled. */
	let busy = $state<string | null>(null);

	async function attempt(action: () => Promise<unknown>) {
		try {
			await action();
		} catch (e) {
			settings.say(String(e), true);
		}
	}

	function toggleCommunity(on: boolean) {
		if (on) trusting = true;
		else void attempt(() => setCommunity(false));
	}

	function trust() {
		trusting = false;
		void attempt(() => setCommunity(true));
	}

	/** Asks the registry for each installed plugin's latest version. */
	async function checkUpdates() {
		checking = true;
		try {
			const listed = await browsePlugins();
			const found: typeof updates = {};
			await Promise.all(
				plugins.view.installed.map(async (manifest) => {
					const entry = listed.find((candidate) => candidate.id === manifest.id);
					if (!entry) return;
					const latest = (await pluginDetails(entry.repo)).manifest;
					if (older(manifest.version, latest.version))
						found[manifest.id] = { version: latest.version, repo: entry.repo };
				})
			);
			updates = found;
			if (Object.keys(found).length === 0) settings.say(m.plugins_updates_none());
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			checking = false;
		}
	}

	async function update(id: string) {
		busy = id;
		await attempt(async () => {
			await adopt(await installPlugin(updates[id].repo, id));
			delete updates[id];
		});
		busy = null;
	}

	async function uninstall(manifest: PluginManifest) {
		removing = null;
		busy = manifest.id;
		await attempt(async () => adopt(await uninstallPlugin(manifest.id)));
		busy = null;
	}
</script>

{#if browsing}
	<PluginBrowser {settings} onback={() => (browsing = false)} />
{:else}
	<div class="flex flex-col gap-6">
		<div class={group}>
			<SettingRow id="community" label={m.plugins_community()} hint={m.plugins_community_hint()}>
				<!-- Each switch shows what is saved, so one the trust warning
				     cancels, or a save that fails, springs back. -->
				<Switch id="community" bind:checked={() => plugins.view.community, toggleCommunity} />
			</SettingRow>
			{#if plugins.view.community}
				<SettingRow label={m.plugins_browse()} hint={m.plugins_browse_hint()}>
					<div class="flex shrink-0 items-center gap-2">
						<Button variant="secondary" size="sm" onclick={checkUpdates} disabled={checking}>
							<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
							{checking ? m.settings_checking() : m.plugins_check_updates()}
						</Button>
						<Button size="sm" onclick={() => (browsing = true)}>
							<StoreIcon />{m.plugins_browse()}
						</Button>
					</div>
				</SettingRow>
			{/if}
		</div>

		{#if plugins.view.community}
			<section class="flex flex-col gap-2">
				<h4 class={section}>{m.plugins_installed()}</h4>
				{#if plugins.view.installed.length === 0}
					<p class={hint}>{m.plugins_installed_none()}</p>
				{:else}
					<div class={group}>
						{#each plugins.view.installed as manifest (manifest.id)}
							{@const id = manifest.id}
							{@const on = plugins.view.enabled.includes(id)}
							{@const status = plugins.status[id]}
							{@const newer = updates[id]}
							<div class="flex items-center justify-between gap-6 px-4 py-3">
								<div class="flex min-w-0 flex-col gap-0.5">
									<div class="flex min-w-0 items-center gap-2">
										<span class="truncate text-sm font-medium">{manifest.name}</span>
										<Badge variant="secondary" class="font-mono">{manifest.version}</Badge>
									</div>
									{#if manifest.author}
										<p class={hint}>{m.plugins_by({ author: manifest.author })}</p>
									{/if}
									{#if manifest.description}
										<p class="{hint} line-clamp-2">{manifest.description}</p>
									{/if}
									{#if status?.state === 'failed'}
										<p class="text-xs text-destructive">
											{m.plugins_failed_because({ error: status.error })}
										</p>
									{/if}
								</div>
								<div class="flex shrink-0 items-center gap-1">
									{#if newer}
										<Button size="sm" onclick={() => update(id)} disabled={busy === id}>
											<DownloadIcon class={busy === id ? 'animate-pulse' : ''} />
											{m.plugins_update_to({ version: newer.version })}
										</Button>
									{/if}
									{#if on}<PluginOptions {settings} {id} name={manifest.name} />{/if}
									<Button
										variant="ghost"
										size="icon-sm"
										onclick={() => (removing = manifest)}
										disabled={busy === id}
										aria-label={m.plugins_uninstall()}
										title={m.plugins_uninstall()}
										class="text-muted-foreground hover:text-destructive"
									>
										<Trash2Icon />
									</Button>
									<Switch
										bind:checked={
											() => on, (next) => void attempt(() => setCommunityPlugin(id, next))
										}
										aria-label={manifest.name}
									/>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</section>
		{/if}
	</div>
{/if}

<Dialog.Root bind:open={trusting}>
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<ShieldAlertIcon class="size-5 text-amber-500" />{m.plugins_trust_title()}
			</Dialog.Title>
			<Dialog.Description>{m.plugins_trust_description()}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (trusting = false)}>{m.common_cancel()}</Button>
			<Button onclick={trust}>{m.plugins_trust_confirm()}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={removing !== null}
	onOpenChange={(open) => {
		if (!open) removing = null;
	}}
>
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title>{m.plugins_uninstall_title({ name: removing?.name ?? '' })}</Dialog.Title>
			<Dialog.Description>{m.plugins_uninstall_description()}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (removing = null)}>{m.common_cancel()}</Button>
			<Button variant="destructive" onclick={() => removing && uninstall(removing)}>
				{m.plugins_uninstall()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
