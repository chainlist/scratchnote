<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
	import {
		checkModelUpdate,
		downloadModel,
		getAliases,
		getSettings,
		MODEL_CHOICES,
		modelInfo,
		modelStatus,
		onModelStatus,
		onModelUpdateProgress,
		onSettingsChanged,
		rebuildIndex,
		setAliases,
		setSettings,
		updateModel,
		type ModelInfo,
		type ModelStatus,
		type ModelVariant,
		type Settings,
		type SettingsView,
		type UpdateCheck
	} from '$lib/api';

	let view = $state<SettingsView | null>(null);
	let draft = $state<Settings>({
		root: '',
		captureHotkey: '',
		hideImmediately: true,
		modelVariant: 'default',
		modelPath: null,
		idleUnloadMinutes: 10
	});
	let customPath = $state('');
	let info = $state<ModelInfo | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let update = $state<UpdateCheck | null>(null);
	let checking = $state(false);
	/** Download progress of an update, null when none is running. */
	let updating = $state<number | null>(null);
	let aliases = $state<{ from: string; to: string }[]>([]);
	let launchAtLogin = $state(false);
	let recording = $state(false);
	let rebuilding = $state(false);
	let message = $state<{ text: string; error: boolean } | null>(null);

	const editable = (s: Settings): Settings => ({
		root: s.root,
		captureHotkey: s.captureHotkey,
		hideImmediately: s.hideImmediately,
		modelVariant: s.modelVariant,
		modelPath: s.modelPath,
		idleUnloadMinutes: s.idleUnloadMinutes
	});
	// Model choices apply as soon as they are picked, so only these wait on
	// the Save button.
	const dirty = $derived(
		view !== null &&
			(draft.root !== view.root ||
				draft.captureHotkey !== view.captureHotkey ||
				draft.hideImmediately !== view.hideImmediately ||
				draft.idleUnloadMinutes !== view.idleUnloadMinutes)
	);
	const restartNeeded = $derived(view !== null && view.root !== view.activeRoot);

	onMount(() => {
		const off = [
			onModelStatus((status) => {
				model = status;
				if (status.state !== 'downloading') void refreshModels();
			}),
			onModelUpdateProgress((percent) => (updating = percent)),
			// A finished download makes that model the one in use.
			onSettingsChanged((settings) => {
				if (!view) return;
				view = { ...view, ...settings };
				draft.modelVariant = settings.modelVariant;
				draft.modelPath = settings.modelPath;
			})
		];
		void (async () => {
			try {
				view = await getSettings();
				draft = editable(view);
				customPath = view.modelPath ?? '';
				aliases = Object.entries(await getAliases()).map(([from, to]) => ({ from, to }));
				launchAtLogin = await isEnabled();
				model = await modelStatus();
				await refreshModels();
			} catch (e) {
				say(String(e), true);
			}
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	async function refreshModels() {
		info = await modelInfo();
	}

	async function download(variant: ModelVariant) {
		try {
			await downloadModel(variant);
			say('Model downloaded and in use.');
		} catch (e) {
			say(String(e), true);
		}
	}

	async function check() {
		checking = true;
		update = null;
		try {
			update = await checkModelUpdate();
		} finally {
			checking = false;
		}
	}

	async function applyUpdate() {
		updating = 0;
		try {
			await updateModel();
			update = null;
			await refreshModels();
			say('Model updated.');
		} catch (e) {
			say(String(e), true);
		} finally {
			updating = null;
		}
	}

	async function useModel(modelVariant: ModelVariant, modelPath: string | null) {
		if (!view) return;
		try {
			view = await setSettings({ ...editable(view), modelVariant, modelPath });
			draft.modelVariant = view.modelVariant;
			draft.modelPath = view.modelPath;
			update = null;
			await refreshModels();
		} catch (e) {
			say(String(e), true);
		}
	}

	const short = (revision: string) => revision.slice(0, 7);

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

				<label class="flex items-center gap-2 text-sm text-neutral-300">
					Unload the model after
					<input
						type="number"
						min="0"
						bind:value={draft.idleUnloadMinutes}
						class="{field} w-20 font-mono"
					/>
					idle minutes
				</label>
				<p class="-mt-3 text-xs text-neutral-500">
					Frees its memory; the next note loads it again. 0 keeps it loaded.
				</p>

				<div>
					<button type="button" onclick={save} disabled={!dirty} class={button}>Save</button>
				</div>

				<label class="flex items-center gap-2 text-sm text-neutral-300">
					<input type="checkbox" checked={launchAtLogin} onchange={toggleLaunchAtLogin} />
					Launch at login
				</label>
			</section>

			<section class="flex flex-col gap-3">
				<h2 class="text-xs font-medium tracking-wide text-neutral-500 uppercase">Model</h2>

				{#each MODEL_CHOICES as choice (choice.variant)}
					{@const record = info?.[choice.variant] ?? null}
					<div class="flex items-center gap-2 text-sm">
						<input
							type="radio"
							name="model"
							id="model-{choice.variant}"
							checked={view.modelPath === null && info?.activeVariant === choice.variant}
							disabled={!record}
							onchange={() => useModel(choice.variant, null)}
						/>
						<label for="model-{choice.variant}" class="flex-1 text-neutral-300">
							{choice.name}
							<span class="text-xs text-neutral-500">{choice.note}</span>
						</label>
						{#if record}
							<span class="font-mono text-xs text-neutral-500" title={record.revision}>
								{short(record.revision)}
							</span>
						{:else}
							<button
								type="button"
								onclick={() => download(choice.variant)}
								disabled={model.state === 'downloading'}
								class={button}
							>
								Download {choice.size}
							</button>
						{/if}
					</div>
				{/each}

				<div class="flex items-center gap-2 text-sm">
					<input
						type="radio"
						name="model"
						id="model-custom"
						checked={view.modelPath !== null}
						disabled={view.modelPath === null && customPath.trim() === ''}
						onchange={() => useModel(view!.modelVariant, customPath)}
					/>
					<label for="model-custom" class="text-neutral-300">Custom GGUF file</label>
				</div>
				<div class="flex gap-2 pl-6">
					<input
						bind:value={customPath}
						placeholder="C:\models\my-model.gguf"
						spellcheck="false"
						class="{field} font-mono"
					/>
					<button
						type="button"
						onclick={() => useModel(view!.modelVariant, customPath)}
						disabled={customPath.trim() === '' || customPath === view.modelPath}
						class={button}
					>
						Use
					</button>
				</div>

				{#if model.state === 'downloading'}
					<div class="h-1.5 overflow-hidden rounded-full bg-neutral-800">
						<div
							class="h-full rounded-full bg-neutral-300 transition-[width] duration-300"
							style="width: {model.percent ?? 0}%"
						></div>
					</div>
				{/if}

				<p class="text-xs text-neutral-500">
					{#if info?.activePath}
						In use: <span class="font-mono">{info.activePath}</span>
					{:else if view.modelPath !== null}
						<span class="text-amber-400">The custom file is missing, so notes stay pending.</span>
					{:else}
						No model installed. Notes stay pending until one is.
					{/if}
				</p>

				{#if info?.activeVariant && view.modelPath === null}
					<div class="flex flex-wrap items-center gap-3">
						<button
							type="button"
							onclick={check}
							disabled={checking || updating !== null}
							class={button}
						>
							{checking ? 'Checking' : 'Check for updates'}
						</button>
						{#if updating !== null}
							<span class="text-xs text-neutral-400">Downloading the update: {updating}%</span>
						{:else if update?.state === 'upToDate'}
							<span class="text-xs text-neutral-400">Up to date ({short(update.revision)}).</span>
						{:else if update?.state === 'newer'}
							<span class="text-xs text-neutral-300">
								{short(update.latest)} is out; you have {short(update.installed)}.
							</span>
							<button type="button" onclick={applyUpdate} class={button}>Update</button>
						{:else if update?.state === 'failed'}
							<span class="text-xs text-amber-400">Could not check: {update.reason}</span>
						{/if}
					</div>
				{/if}
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
