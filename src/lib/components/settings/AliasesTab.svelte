<script lang="ts">
	import { onMount } from 'svelte';
	import { getAliases, setAliases } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import TagIcon from '@lucide/svelte/icons/tag';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '$lib/paraglide/messages';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	let aliases = $state<{ from: string; to: string }[]>([]);

	const rows = (saved: Record<string, string>) =>
		Object.entries(saved).map(([from, to]) => ({ from, to }));

	onMount(async () => {
		try {
			aliases = rows(await getAliases());
		} catch (e) {
			settings.say(String(e), true);
		}
	});

	async function save() {
		const pairs = aliases.filter((a) => a.from.trim() !== '' || a.to.trim() !== '');
		try {
			aliases = rows(await setAliases(Object.fromEntries(pairs.map((a) => [a.from, a.to]))));
			settings.say(m.settings_aliases_saved());
		} catch (e) {
			settings.say(String(e), true);
		}
	}
</script>

<div class="flex flex-col gap-4">
	{#if aliases.length === 0}
		<div
			class="flex flex-col items-center gap-1 rounded-lg border border-dashed px-4 py-8 text-center"
		>
			<TagIcon class="size-5 text-muted-foreground" />
			<p class="text-sm font-medium">{m.settings_aliases_empty()}</p>
			<p class={hint}>{m.settings_aliases_empty_hint()}</p>
		</div>
	{:else}
		<div class={group}>
			{#each aliases as alias, i (i)}
				<div class="flex items-center gap-2 px-3 py-2">
					<Input bind:value={alias.from} placeholder="k8s" spellcheck="false" class="font-mono" />
					<ArrowRightIcon class="size-4 shrink-0 text-muted-foreground" />
					<Input
						bind:value={alias.to}
						placeholder="kubernetes"
						spellcheck="false"
						class="font-mono"
					/>
					<Button
						variant="ghost"
						size="icon"
						onclick={() => aliases.splice(i, 1)}
						aria-label={m.settings_alias_remove()}
					>
						<XIcon />
					</Button>
				</div>
			{/each}
		</div>
	{/if}
	<div class="flex justify-between gap-2">
		<Button variant="outline" onclick={() => aliases.push({ from: '', to: '' })}>
			<PlusIcon />
			{m.settings_alias_add()}
		</Button>
		<Button onclick={save}>{m.settings_aliases_save()}</Button>
	</div>
</div>
