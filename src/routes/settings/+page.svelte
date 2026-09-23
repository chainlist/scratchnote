<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
	import {
		getAliases,
		getSettings,
		rebuildIndex,
		setAliases,
		setSettings,
		type SettingsView
	} from '$lib/api';

	let view = $state<SettingsView | null>(null);
	let draft = $state({ root: '', captureHotkey: '', hideImmediately: true });
	let aliases = $state<{ from: string; to: string }[]>([]);
	let launchAtLogin = $state(false);
	let recording = $state(false);
	let rebuilding = $state(false);
	let message = $state<{ text: string; error: boolean } | null>(null);

	const dirty = $derived(
		view !== null &&
			(draft.root !== view.root ||
				draft.captureHotkey !== view.captureHotkey ||
				draft.hideImmediately !== view.hideImmediately)
	);
	const restartNeeded = $derived(view !== null && view.root !== view.activeRoot);

	onMount(async () => {
		try {
			view = await getSettings();
			draft = {
				root: view.root,
				captureHotkey: view.captureHotkey,
				hideImmediately: view.hideImmediately
			};
			aliases = Object.entries(await getAliases()).map(([from, to]) => ({ from, to }));
			launchAtLogin = await isEnabled();
		} catch (e) {
			say(String(e), true);
		}
	});

	function say(text: string, error = false) {
		message = { text, error };
	}

	async function save() {
		try {
			view = await setSettings($state.snapshot(draft));
			say('Saved.');
		} catch (e) {
			say(String(e), true);
		}
	}

	async function saveAliases() {
		const pairs = aliases.filter((a) => a.from.trim() !== '' || a.to.trim() !== '');
		try {
			const saved = await setAliases(Object.fromEntries(pairs.map((a) => [a.from, a.to])));
			aliases = Object.entries(saved).map(([from, to]) => ({ from, to }));
			say('Aliases saved.');
		} catch (e) {
			say(String(e), true);
		}
	}

	async function toggleLaunchAtLogin() {
		try {
			if (launchAtLogin) await disable();
			else await enable();
			launchAtLogin = await isEnabled();
		} catch (e) {
			say(String(e), true);
		}
	}

	async function rebuild() {
		rebuilding = true;
		try {
			const count = await rebuildIndex();
			say(`Index rebuilt from the markdown: ${count} notes.`);
		} catch (e) {
			say(String(e), true);
		} finally {
			rebuilding = false;
		}
	}

	const isMac = navigator.userAgent.includes('Mac');

	/** Turns a key press into the accelerator syntax the global-shortcut plugin reads. */
	function recordHotkey(event: KeyboardEvent) {
		event.preventDefault();
		if (event.key === 'Escape') {
			recording = false;
			return;
		}
		if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;

		const mods: string[] = [];
		if (isMac ? event.metaKey : event.ctrlKey) mods.push('CommandOrControl');
		if (isMac ? event.ctrlKey : event.metaKey) mods.push(isMac ? 'Control' : 'Super');
		if (event.altKey) mods.push('Alt');
		if (event.shiftKey) mods.push('Shift');
		// A bare key would fire on every keystroke typed anywhere.
		if (mods.length === 0) return;

		const key = event.code.replace(/^Key/, '').replace(/^Digit/, '');
		draft.captureHotkey = [...mods, key].join('+');
		recording = false;
	}

	const field =
		'w-full rounded border border-neutral-800 bg-neutral-900 px-2 py-1.5 text-sm text-neutral-100 placeholder:text-neutral-600 focus:border-neutral-600 focus:outline-none';
	const button =
		'cursor-pointer rounded bg-neutral-800 px-3 py-1.5 text-sm text-neutral-200 hover:bg-neutral-700 disabled:cursor-default disabled:opacity-50';
</script>

<div class="h-screen overflow-y-auto bg-neutral-950 text-neutral-100">
	<div class="mx-auto flex max-w-2xl flex-col gap-8 p-6">
		<header class="flex items-baseline justify-between">
			<h1 class="text-lg font-semibold">Settings</h1>
			<a href={resolve('/')} class="text-sm text-neutral-400 hover:text-neutral-200"
				>Back to notes</a
			>
		</header>

		{#if message}
			<p
				class="rounded border p-3 text-sm {message.error
					? 'border-red-900 bg-red-950 text-red-300'
					: 'border-neutral-800 bg-neutral-900 text-neutral-300'}"
			>
				{message.text}
			</p>
		{/if}

		{#if view}
			<section class="flex flex-col gap-4">
				<h2 class="text-xs font-medium tracking-wide text-neutral-500 uppercase">General</h2>

				<label class="flex flex-col gap-1 text-sm">
					<span class="text-neutral-300">Notes folder</span>
					<input bind:value={draft.root} spellcheck="false" class="{field} font-mono" />
					{#if restartNeeded}
						<span class="text-xs text-amber-400">
							Saved. Restart Scratchnote to switch from {view.activeRoot}.
						</span>
					{/if}
				</label>

				<label class="flex flex-col gap-1 text-sm">
					<span class="text-neutral-300">Capture hotkey</span>
					<input
						readonly
						value={recording ? 'Press the keys, Esc to cancel' : draft.captureHotkey}
						onfocus={() => (recording = true)}
						onblur={() => (recording = false)}
						onkeydown={recordHotkey}
						class="{field} cursor-pointer font-mono {recording ? 'text-neutral-500' : ''}"
					/>
				</label>

				<label class="flex items-center gap-2 text-sm text-neutral-300">
					<input type="checkbox" bind:checked={draft.hideImmediately} />
					Hide the capture window as soon as a note is saved
				</label>
				{#if !draft.hideImmediately}
					<p class="-mt-3 pl-6 text-xs text-neutral-500">It shows "Saved" for a second first.</p>
				{/if}

				<div>
					<button type="button" onclick={save} disabled={!dirty} class={button}>Save</button>
				</div>

				<label class="flex items-center gap-2 text-sm text-neutral-300">
					<input type="checkbox" checked={launchAtLogin} onchange={toggleLaunchAtLogin} />
					Launch at login
				</label>
			</section>

			<section class="flex flex-col gap-3">
				<h2 class="text-xs font-medium tracking-wide text-neutral-500 uppercase">Tag aliases</h2>
				<p class="text-xs text-neutral-500">
					A tag on the left is written as the tag on the right, for new tags and for #tag searches.
					Tags already in your notes are not rewritten.
				</p>
				{#each aliases as alias, i (i)}
					<div class="flex items-center gap-2">
						<input
							bind:value={alias.from}
							placeholder="k8s"
							spellcheck="false"
							class="{field} font-mono"
						/>
						<span class="text-neutral-600">→</span>
						<input
							bind:value={alias.to}
							placeholder="kubernetes"
							spellcheck="false"
							class="{field} font-mono"
						/>
						<button
							type="button"
							onclick={() => aliases.splice(i, 1)}
							aria-label="Remove alias"
							class="cursor-pointer px-2 text-neutral-500 hover:text-neutral-200">×</button
						>
					</div>
				{/each}
				<div class="flex gap-2">
					<button type="button" onclick={() => aliases.push({ from: '', to: '' })} class={button}>
						Add alias
					</button>
					<button type="button" onclick={saveAliases} class={button}>Save aliases</button>
				</div>
			</section>

			<section class="flex flex-col gap-3">
				<h2 class="text-xs font-medium tracking-wide text-neutral-500 uppercase">Index</h2>
				<p class="text-xs text-neutral-500">
					Reparse every daily file. The index is only a cache of the markdown, so this is always
					safe.
				</p>
				<div>
					<button type="button" onclick={rebuild} disabled={rebuilding} class={button}>
						{rebuilding ? 'Rebuilding' : 'Rebuild index'}
					</button>
				</div>
			</section>
		{/if}
	</div>
</div>
