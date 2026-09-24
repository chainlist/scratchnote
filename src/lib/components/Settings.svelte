<script lang="ts">
	import { onMount } from 'svelte';
	import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
	import * as Alert from '$lib/components/ui/alert';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Progress } from '$lib/components/ui/progress';
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import { Switch } from '$lib/components/ui/switch';
	import * as Tabs from '$lib/components/ui/tabs';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import BrainIcon from '@lucide/svelte/icons/brain';
	import CheckIcon from '@lucide/svelte/icons/check';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import PaletteIcon from '@lucide/svelte/icons/palette';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import SlidersHorizontalIcon from '@lucide/svelte/icons/sliders-horizontal';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import TagIcon from '@lucide/svelte/icons/tag';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import XIcon from '@lucide/svelte/icons/x';
	import {
		checkModelUpdate,
		downloadModel,
		getAliases,
		getSettings,
		gpuDevices,
		MODEL_CHOICES,
		modelInfo,
		modelStatus,
		onModelStatus,
		onModelUpdateProgress,
		onSettingsChanged,
		rebuildIndex,
		regenerateAll,
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
	import { ACCENTS, DEFAULT_APPEARANCE, FONT_SIZES, RADII, THEMES } from '$lib/appearance';

	let view = $state<SettingsView | null>(null);
	let draft = $state<Settings>({
		root: '',
		captureHotkey: '',
		hideImmediately: true,
		modelVariant: 'default',
		modelPath: null,
		idleUnloadMinutes: 10,
		useGpu: true,
		...DEFAULT_APPEARANCE
	});
	let customPath = $state('');
	let info = $state<ModelInfo | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let update = $state<UpdateCheck | null>(null);
	let checking = $state(false);
	/** Download progress of an update, null when none is running. */
	let updating = $state<number | null>(null);
	let gpus = $state<string[]>([]);
	let aliases = $state<{ from: string; to: string }[]>([]);
	let launchAtLogin = $state(false);
	let recording = $state(false);
	let rebuilding = $state(false);
	let regenerating = $state(false);
	/** The regenerate button asks once more before it rewrites every note. */
	let confirmRegenerate = $state(false);
	let message = $state<{ text: string; error: boolean } | null>(null);

	const editable = (s: Settings): Settings => ({
		root: s.root,
		captureHotkey: s.captureHotkey,
		hideImmediately: s.hideImmediately,
		modelVariant: s.modelVariant,
		modelPath: s.modelPath,
		idleUnloadMinutes: s.idleUnloadMinutes,
		useGpu: s.useGpu,
		accentColor: s.accentColor,
		fontSize: s.fontSize,
		radius: s.radius,
		theme: s.theme
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
				draft.accentColor = settings.accentColor;
				draft.fontSize = settings.fontSize;
				draft.radius = settings.radius;
				draft.theme = settings.theme;
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
				gpus = await gpuDevices();
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

	/** Applies at once: the model is unloaded and the next note reloads it. */
	async function setGpu(useGpu: boolean) {
		if (!view) return;
		try {
			view = await setSettings({ ...editable(view), useGpu });
			draft.useGpu = view.useGpu;
		} catch (e) {
			say(String(e), true);
		}
	}

	/** Applies at once: the layout of every window repaints on settings-changed. */
	async function setAppearance(appearance: Partial<Settings>) {
		if (!view) return;
		try {
			view = await setSettings({ ...editable(view), ...appearance });
		} catch (e) {
			say(String(e), true);
		}
	}

	const appearanceIsDefault = $derived(
		view !== null &&
			view.accentColor === DEFAULT_APPEARANCE.accentColor &&
			view.fontSize === DEFAULT_APPEARANCE.fontSize &&
			view.radius === DEFAULT_APPEARANCE.radius &&
			view.theme === DEFAULT_APPEARANCE.theme
	);

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

	async function regenerate() {
		confirmRegenerate = false;
		regenerating = true;
		try {
			const count = await regenerateAll();
			say(`${count} notes queued. The model works through them in the background.`);
		} catch (e) {
			say(String(e), true);
		} finally {
			regenerating = false;
		}
	}

	/**
	 * Escape cancels a recording rather than closing the dialog. Caught on the
	 * window's capture phase, it never reaches the dialog's document listener.
	 */
	function cancelRecording(event: KeyboardEvent) {
		if (!recording || event.key !== 'Escape') return;
		event.preventDefault();
		event.stopPropagation();
		recording = false;
	}

	const isMac = navigator.userAgent.includes('Mac');

	/** Turns a key press into the accelerator syntax the global-shortcut plugin reads. */
	function recordHotkey(event: KeyboardEvent) {
		// Once cancelled, keys (Escape to close the dialog) pass through.
		if (!recording) return;
		event.preventDefault();
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

	const modelChoice = $derived(
		view && view.modelPath !== null ? 'custom' : (info?.activeVariant ?? '')
	);

	function pickModel(value: string) {
		if (!view) return;
		if (value === 'custom') void useModel(view.modelVariant, customPath);
		else void useModel(value as ModelVariant, null);
	}

	/** Shows an accelerator the way the OS spells it, e.g. Ctrl+Shift+Space. */
	const prettyHotkey = (accelerator: string) =>
		accelerator.replace('CommandOrControl', isMac ? '⌘' : 'Ctrl').replace('Super', 'Win');

	const tabs = [
		{ value: 'general', label: 'General', icon: SlidersHorizontalIcon },
		{ value: 'appearance', label: 'Appearance', icon: PaletteIcon },
		{ value: 'model', label: 'Model', icon: BrainIcon },
		{ value: 'tags', label: 'Tag aliases', icon: TagIcon },
		{ value: 'index', label: 'Index', icon: DatabaseIcon }
	];

	const group = 'divide-y rounded-lg border bg-card';
	const row = 'flex items-center justify-between gap-6 px-4 py-3';
	const hint = 'text-xs text-muted-foreground';
	/** A selectable model card; highlighted while its radio is checked. */
	/** One option of a segmented picker, filled while selected. */
	const segment = (selected: boolean) =>
		`h-7 rounded-md px-3 text-xs font-medium transition-colors ${
			selected
				? 'bg-primary text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-foreground'
		}`;
	const card =
		'gap-3 rounded-lg border bg-card px-4 py-3 font-normal transition-colors has-data-checked:border-primary has-data-checked:bg-primary/5';
</script>

<svelte:window onkeydowncapture={cancelRecording} />

{#snippet header(title: string, description: string)}
	<div class="mb-5 flex flex-col gap-1">
		<h3 class="text-base font-semibold">{title}</h3>
		<p class="text-sm text-muted-foreground">{description}</p>
	</div>
{/snippet}

<Tabs.Root value="general" orientation="vertical" class="h-full min-h-0 gap-0">
	<aside class="flex w-48 shrink-0 flex-col gap-4 border-r bg-muted/40 p-3">
		<Dialog.Title class="px-2 pt-1">Settings</Dialog.Title>
		<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
			{#each tabs as t (t.value)}
				<Tabs.Trigger value={t.value} class="h-8 w-full flex-none justify-start gap-2 px-2">
					<t.icon />
					{t.label}
				</Tabs.Trigger>
			{/each}
		</Tabs.List>
	</aside>

	<div class="flex min-w-0 flex-1 flex-col">
		<div class="flex-1 overflow-y-auto p-6">
			{#if message}
				<Alert.Root variant={message.error ? 'destructive' : 'default'} class="mb-5">
					{#if message.error}<CircleAlertIcon />{:else}<CircleCheckIcon />{/if}
					<Alert.Description>{message.text}</Alert.Description>
				</Alert.Root>
			{/if}

			{#if view}
				<Tabs.Content value="general">
					{@render header('General', 'Where your notes live and how you capture them.')}

					<div class={group}>
						<div class="flex flex-col gap-2 px-4 py-3">
							<div class="flex flex-col gap-0.5">
								<Label for="root">Notes folder</Label>
								<p class={hint}>One markdown file per day lives here.</p>
							</div>
							<Input id="root" bind:value={draft.root} spellcheck="false" class="font-mono" />
							{#if restartNeeded}
								<p class="text-xs text-amber-500">
									Saved. Restart Scratchnote to switch from {view.activeRoot}.
								</p>
							{/if}
						</div>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<Label for="hotkey">Capture hotkey</Label>
								<p class={hint}>Opens the capture window from anywhere.</p>
							</div>
							<Input
								id="hotkey"
								readonly
								value={recording ? 'Press keys, Esc cancels' : prettyHotkey(draft.captureHotkey)}
								onfocus={() => (recording = true)}
								onblur={() => (recording = false)}
								onkeydown={recordHotkey}
								class="w-56 cursor-pointer text-center font-mono {recording
									? 'border-primary text-muted-foreground'
									: ''}"
							/>
						</div>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<Label for="hide">Hide after saving</Label>
								<p class={hint}>
									{draft.hideImmediately
										? 'The capture window closes as soon as a note is saved.'
										: 'The capture window shows "Saved" for a second first.'}
								</p>
							</div>
							<Switch id="hide" bind:checked={draft.hideImmediately} />
						</div>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<Label for="login">Launch at login</Label>
								<p class={hint}>Start Scratchnote when you sign in.</p>
							</div>
							<Switch id="login" checked={launchAtLogin} onCheckedChange={toggleLaunchAtLogin} />
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="appearance">
					{@render header('Appearance', 'Colors and sizes. Changes apply right away.')}

					<div class="flex flex-col gap-4">
						<div class={group}>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">Theme</span>
									<p class={hint}>System follows your OS setting.</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label="Theme"
								>
									{#each THEMES as theme (theme.name)}
										<button
											type="button"
											role="radio"
											aria-checked={view.theme === theme.name}
											onclick={() => setAppearance({ theme: theme.name })}
											class={segment(view.theme === theme.name)}
										>
											{theme.label}
										</button>
									{/each}
								</div>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">Accent color</span>
									<p class={hint}>Buttons, switches and focus rings.</p>
								</div>
								<div class="flex gap-2" role="radiogroup" aria-label="Accent color">
									{#each ACCENTS as accent (accent.name)}
										{@const selected = view.accentColor === accent.name}
										<button
											type="button"
											role="radio"
											aria-checked={selected}
											aria-label={accent.label}
											title={accent.label}
											onclick={() => setAppearance({ accentColor: accent.name })}
											class="flex size-7 cursor-pointer items-center justify-center rounded-full ring-offset-2 ring-offset-card transition {selected
												? 'ring-2 ring-foreground'
												: 'hover:ring-2 hover:ring-border'}"
											style="background: {accent.swatch}"
										>
											{#if selected}
												<CheckIcon class="size-4" style="color: {accent.foreground}" />
											{/if}
										</button>
									{/each}
								</div>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">Text size</span>
									<p class={hint}>Scales the whole interface, notes included.</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label="Text size"
								>
									{#each FONT_SIZES as size (size.px)}
										<button
											type="button"
											role="radio"
											aria-checked={view.fontSize === size.px}
											onclick={() => setAppearance({ fontSize: size.px })}
											class={segment(view.fontSize === size.px)}
										>
											{size.label}
										</button>
									{/each}
								</div>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">Corner radius</span>
									<p class={hint}>How rounded cards, buttons and inputs are.</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label="Corner radius"
								>
									{#each RADII as radius (radius.rem)}
										<button
											type="button"
											role="radio"
											aria-checked={view.radius === radius.rem}
											onclick={() => setAppearance({ radius: radius.rem })}
											class={segment(view.radius === radius.rem)}
										>
											{radius.label}
										</button>
									{/each}
								</div>
							</div>
						</div>
						<div class="flex justify-end">
							<Button
								variant="outline"
								size="sm"
								onclick={() => setAppearance(DEFAULT_APPEARANCE)}
								disabled={appearanceIsDefault}
							>
								Reset to defaults
							</Button>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="model">
					{@render header('Model', 'The local model that tags and enriches your notes.')}

					<div class="flex flex-col gap-6">
						<RadioGroup.Root value={modelChoice} onValueChange={pickModel} class="gap-2">
							{#each MODEL_CHOICES as choice (choice.variant)}
								{@const record = info?.[choice.variant] ?? null}
								<Label for="model-{choice.variant}" class="{card} flex items-center">
									<RadioGroup.Item
										value={choice.variant}
										id="model-{choice.variant}"
										disabled={!record}
									/>
									<div class="flex flex-1 flex-col gap-0.5">
										<span class="flex items-center gap-2">
											{choice.name}
											{#if info?.activeVariant === choice.variant && view.modelPath === null}
												<Badge>In use</Badge>
											{/if}
										</span>
										<span class={hint}>{choice.note}</span>
									</div>
									{#if record}
										<Badge variant="outline" class="font-mono" title={record.revision}>
											{short(record.revision)}
										</Badge>
									{:else}
										<Button
											variant="secondary"
											size="sm"
											onclick={() => download(choice.variant)}
											disabled={model.state === 'downloading'}
										>
											<DownloadIcon />
											{choice.size}
										</Button>
									{/if}
								</Label>
							{/each}

							<div class="{card} flex flex-col">
								<div class="flex items-center gap-3">
									<RadioGroup.Item
										value="custom"
										id="model-custom"
										disabled={view.modelPath === null && customPath.trim() === ''}
									/>
									<Label for="model-custom" class="flex-1 font-normal">
										Custom GGUF file
										{#if view.modelPath !== null}<Badge>In use</Badge>{/if}
									</Label>
								</div>
								<div class="flex gap-2 pl-7">
									<Input
										bind:value={customPath}
										placeholder="C:\models\my-model.gguf"
										spellcheck="false"
										class="font-mono"
									/>
									<Button
										variant="secondary"
										onclick={() => useModel(view!.modelVariant, customPath)}
										disabled={customPath.trim() === '' || customPath === view.modelPath}
									>
										Use
									</Button>
								</div>
							</div>
						</RadioGroup.Root>

						{#if model.state === 'downloading'}
							<div class="flex flex-col gap-2">
								<p class={hint}>Downloading the model: {model.percent ?? 0}%</p>
								<Progress value={model.percent ?? 0} />
							</div>
						{/if}

						<p class={hint}>
							{#if info?.activePath}
								In use: <span class="font-mono break-all">{info.activePath}</span>
							{:else if view.modelPath !== null}
								<span class="text-amber-500">
									The custom file is missing, so notes stay pending.
								</span>
							{:else}
								No model installed. Notes stay pending until one is.
							{/if}
						</p>

						{#if info?.activeVariant && view.modelPath === null}
							<div class={group}>
								<div class={row}>
									<div class="flex flex-col gap-0.5">
										<span class="text-sm font-medium">Updates</span>
										<p class={hint}>
											{#if updating !== null}
												Downloading the update: {updating}%
											{:else if update?.state === 'upToDate'}
												Up to date ({short(update.revision)}).
											{:else if update?.state === 'newer'}
												{short(update.latest)} is out; you have {short(update.installed)}.
											{:else if update?.state === 'failed'}
												<span class="text-amber-500">Could not check: {update.reason}</span>
											{:else}
												Look for a newer revision of this model.
											{/if}
										</p>
									</div>
									{#if update?.state === 'newer' && updating === null}
										<Button size="sm" onclick={applyUpdate}>
											<DownloadIcon />
											Update
										</Button>
									{:else}
										<Button
											variant="secondary"
											size="sm"
											onclick={check}
											disabled={checking || updating !== null}
										>
											<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
											{checking ? 'Checking' : 'Check for updates'}
										</Button>
									{/if}
								</div>
								{#if updating !== null}
									<div class="px-4 py-3"><Progress value={updating} /></div>
								{/if}
							</div>
						{/if}

						<div class={group}>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<Label for="gpu">Use GPU acceleration</Label>
									<p class={hint}>
										{#if gpus.length === 0}
											No supported GPU found, so the model runs on the CPU.
										{:else}
											{gpus.join(', ')}. Much faster and lighter on the CPU.
										{/if}
									</p>
								</div>
								<Switch
									id="gpu"
									checked={view.useGpu && gpus.length > 0}
									disabled={gpus.length === 0}
									onCheckedChange={setGpu}
								/>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<Label for="idle">Unload when idle</Label>
									<p class={hint}>
										Frees its memory; the next note loads it again. 0 keeps it loaded.
									</p>
								</div>
								<div class="flex items-center gap-2">
									<Input
										id="idle"
										type="number"
										min="0"
										bind:value={draft.idleUnloadMinutes}
										class="w-20 text-right font-mono"
									/>
									<span class={hint}>min</span>
								</div>
							</div>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="tags">
					{@render header(
						'Tag aliases',
						'A tag on the left is written as the tag on the right, for new tags and for #tag searches. Tags already in your notes are not rewritten.'
					)}

					<div class="flex flex-col gap-4">
						{#if aliases.length === 0}
							<div
								class="flex flex-col items-center gap-1 rounded-lg border border-dashed px-4 py-8 text-center"
							>
								<TagIcon class="size-5 text-muted-foreground" />
								<p class="text-sm font-medium">No aliases yet</p>
								<p class={hint}>Map k8s to kubernetes, js to javascript, and so on.</p>
							</div>
						{:else}
							<div class={group}>
								{#each aliases as alias, i (i)}
									<div class="flex items-center gap-2 px-3 py-2">
										<Input
											bind:value={alias.from}
											placeholder="k8s"
											spellcheck="false"
											class="font-mono"
										/>
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
											aria-label="Remove alias"
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
								Add alias
							</Button>
							<Button onclick={saveAliases}>Save aliases</Button>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="index">
					{@render header('Index', 'The search index is only a cache of your markdown files.')}

					<div class={group}>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<span class="text-sm font-medium">Rebuild index</span>
								<p class={hint}>Reparse every daily file. Always safe to run.</p>
							</div>
							<Button variant="secondary" size="sm" onclick={rebuild} disabled={rebuilding}>
								<RefreshCwIcon class={rebuilding ? 'animate-spin' : ''} />
								{rebuilding ? 'Rebuilding' : 'Rebuild'}
							</Button>
						</div>
					</div>

					<div
						class="mt-6 flex flex-col gap-3 rounded-lg border border-amber-500/40 bg-amber-500/5 px-4 py-3"
					>
						<div class="flex gap-3">
							<TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-amber-500" />
							<div class="flex flex-col gap-1">
								<span class="text-sm font-medium">Regenerate all notes</span>
								<p class={hint}>
									Clears the subject, summary and tags of every note, hand edits included, then runs
									the model again on each one. The tag list starts empty and fills back in as notes
									are done. This cannot be undone, and it takes a while on a large journal.
								</p>
								{#if !info?.activePath}
									<p class="text-xs text-amber-500">Install a model first.</p>
								{/if}
							</div>
						</div>
						<div class="flex justify-end gap-2">
							{#if confirmRegenerate}
								<Button variant="ghost" size="sm" onclick={() => (confirmRegenerate = false)}>
									Cancel
								</Button>
								<Button variant="destructive" size="sm" onclick={regenerate}>
									Yes, regenerate everything
								</Button>
							{:else}
								<Button
									variant="outline"
									size="sm"
									onclick={() => (confirmRegenerate = true)}
									disabled={regenerating || !info?.activePath}
								>
									<SparklesIcon />
									{regenerating ? 'Queuing' : 'Regenerate all'}
								</Button>
							{/if}
						</div>
					</div>
				</Tabs.Content>
			{/if}
		</div>

		{#if dirty}
			<div class="flex items-center justify-between gap-4 border-t bg-muted/40 px-6 py-3">
				<p class={hint}>You have unsaved changes.</p>
				<Button onclick={save}>Save changes</Button>
			</div>
		{/if}
	</div>
</Tabs.Root>
