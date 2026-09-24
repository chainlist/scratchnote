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
	import * as Select from '$lib/components/ui/select';
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
	import { ACCENTS, DEFAULT_APPEARANCE, FONT_SIZES, FONTS, RADII, THEMES } from '$lib/appearance';
	import { LANGUAGE_NAMES, LANGUAGES, parts, slot, type Language } from '$lib/i18n.svelte';
	import { m } from '$lib/paraglide/messages';

	let view = $state<SettingsView | null>(null);
	let draft = $state<Settings>({
		root: '',
		captureHotkey: '',
		hideImmediately: true,
		modelEnabled: true,
		modelVariant: 'default',
		modelPath: null,
		idleUnloadMinutes: 10,
		useGpu: true,
		...DEFAULT_APPEARANCE,
		language: 'system'
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
		modelEnabled: s.modelEnabled,
		modelVariant: s.modelVariant,
		modelPath: s.modelPath,
		idleUnloadMinutes: s.idleUnloadMinutes,
		useGpu: s.useGpu,
		accentColor: s.accentColor,
		fontFamily: s.fontFamily,
		fontSize: s.fontSize,
		radius: s.radius,
		theme: s.theme,
		language: s.language
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
				draft.modelEnabled = settings.modelEnabled;
				draft.modelVariant = settings.modelVariant;
				draft.modelPath = settings.modelPath;
				draft.accentColor = settings.accentColor;
				draft.fontFamily = settings.fontFamily;
				draft.fontSize = settings.fontSize;
				draft.radius = settings.radius;
				draft.theme = settings.theme;
				draft.language = settings.language;
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

	/** Applies at once: off unloads the model, on lets queued notes through. */
	async function setModelEnabled(modelEnabled: boolean) {
		if (!view) return;
		try {
			view = await setSettings({ ...editable(view), modelEnabled });
			draft.modelEnabled = view.modelEnabled;
		} catch (e) {
			say(String(e), true);
		}
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
			view.fontFamily === DEFAULT_APPEARANCE.fontFamily &&
			view.fontSize === DEFAULT_APPEARANCE.fontSize &&
			view.radius === DEFAULT_APPEARANCE.radius &&
			view.theme === DEFAULT_APPEARANCE.theme
	);

	async function download(variant: ModelVariant) {
		try {
			await downloadModel(variant);
			say(m.settings_model_downloaded());
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
			say(m.settings_model_updated());
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

	/** Languages are named in their own tongue; only System follows the open one. */
	const languageName = (language: Language) =>
		language === 'system' ? m.settings_language_system() : LANGUAGE_NAMES[language];

	function say(text: string, error = false) {
		message = { text, error };
	}

	async function save() {
		try {
			view = await setSettings($state.snapshot(draft));
			say(m.settings_saved());
		} catch (e) {
			say(String(e), true);
		}
	}

	async function saveAliases() {
		const pairs = aliases.filter((a) => a.from.trim() !== '' || a.to.trim() !== '');
		try {
			const saved = await setAliases(Object.fromEntries(pairs.map((a) => [a.from, a.to])));
			aliases = Object.entries(saved).map(([from, to]) => ({ from, to }));
			say(m.settings_aliases_saved());
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
			say(m.settings_rebuilt({ count }));
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
			say(m.settings_regenerate_queued({ count }));
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
		{ value: 'general', label: m.settings_tab_general, icon: SlidersHorizontalIcon },
		{ value: 'appearance', label: m.settings_tab_appearance, icon: PaletteIcon },
		{ value: 'model', label: m.settings_tab_model, icon: BrainIcon },
		{ value: 'tags', label: m.settings_tab_tags, icon: TagIcon },
		{ value: 'index', label: m.settings_tab_index, icon: DatabaseIcon }
	];

	const group = 'divide-y rounded-lg border bg-card';
	const row = 'flex items-center justify-between gap-6 px-4 py-3';
	const hint = 'text-xs text-muted-foreground';
	/** A selectable model card; highlighted while its radio is checked. */
	/** One option of a segmented picker, filled while selected. */
	const segment = (selected: boolean) =>
		`h-7 rounded-md px-3 text-xs font-medium whitespace-nowrap transition-colors ${
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
		<Dialog.Title class="px-2 pt-1">{m.common_settings()}</Dialog.Title>
		<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
			{#each tabs as t (t.value)}
				<Tabs.Trigger value={t.value} class="h-8 w-full flex-none justify-start gap-2 px-2">
					<t.icon />
					{t.label()}
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
					{@render header(m.settings_tab_general(), m.settings_general_description())}

					<div class={group}>
						<div class="flex flex-col gap-2 px-4 py-3">
							<div class="flex flex-col gap-0.5">
								<Label for="root">{m.settings_root()}</Label>
								<p class={hint}>{m.settings_root_hint()}</p>
							</div>
							<Input id="root" bind:value={draft.root} spellcheck="false" class="font-mono" />
							{#if restartNeeded}
								<p class="text-xs text-amber-500">
									{m.settings_restart_needed({ path: view.activeRoot })}
								</p>
							{/if}
						</div>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<Label for="hotkey">{m.settings_hotkey()}</Label>
								<p class={hint}>{m.settings_hotkey_hint()}</p>
							</div>
							<Input
								id="hotkey"
								readonly
								value={recording
									? m.settings_hotkey_recording()
									: prettyHotkey(draft.captureHotkey)}
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
								<Label for="hide">{m.settings_hide()}</Label>
								<p class={hint}>
									{draft.hideImmediately ? m.settings_hide_on() : m.settings_hide_off()}
								</p>
							</div>
							<Switch id="hide" bind:checked={draft.hideImmediately} />
						</div>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<Label for="login">{m.settings_login()}</Label>
								<p class={hint}>{m.settings_login_hint()}</p>
							</div>
							<Switch id="login" checked={launchAtLogin} onCheckedChange={toggleLaunchAtLogin} />
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="appearance">
					{@render header(m.settings_tab_appearance(), m.settings_appearance_description())}

					<div class="flex flex-col gap-4">
						<div class={group}>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">{m.settings_language()}</span>
									<p class={hint}>{m.settings_language_hint()}</p>
								</div>
								<Select.Root
									type="single"
									value={view.language}
									onValueChange={(language) => setAppearance({ language: language as Language })}
								>
									<Select.Trigger size="sm" class="w-40" aria-label={m.settings_language()}>
										{languageName(view.language)}
									</Select.Trigger>
									<Select.Content>
										{#each LANGUAGES as language (language)}
											<Select.Item value={language} label={languageName(language)} />
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">{m.settings_theme()}</span>
									<p class={hint}>{m.settings_theme_hint()}</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label={m.settings_theme()}
								>
									{#each THEMES as theme (theme.name)}
										<button
											type="button"
											role="radio"
											aria-checked={view.theme === theme.name}
											onclick={() => setAppearance({ theme: theme.name })}
											class={segment(view.theme === theme.name)}
										>
											{theme.label()}
										</button>
									{/each}
								</div>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">{m.settings_accent()}</span>
									<p class={hint}>{m.settings_accent_hint()}</p>
								</div>
								<div class="flex gap-2" role="radiogroup" aria-label={m.settings_accent()}>
									{#each ACCENTS as accent (accent.name)}
										{@const selected = view.accentColor === accent.name}
										<button
											type="button"
											role="radio"
											aria-checked={selected}
											aria-label={accent.label()}
											title={accent.label()}
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
									<span class="text-sm font-medium">{m.settings_font()}</span>
									<p class={hint}>{m.settings_font_hint()}</p>
								</div>
								<Select.Root
									type="single"
									value={view.fontFamily}
									onValueChange={(fontFamily) => setAppearance({ fontFamily })}
								>
									<Select.Trigger size="sm" class="w-48" aria-label={m.settings_font()}>
										{(FONTS.find((f) => f.name === view?.fontFamily) ?? FONTS[0]).label()}
									</Select.Trigger>
									<Select.Content>
										{#each FONTS as font (font.name)}
											<Select.Item value={font.name} label={font.label()}>
												<span style="font-family: {font.family}">{font.label()}</span>
											</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">{m.settings_text_size()}</span>
									<p class={hint}>{m.settings_text_size_hint()}</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label={m.settings_text_size()}
								>
									{#each FONT_SIZES as size (size.px)}
										<button
											type="button"
											role="radio"
											aria-checked={view.fontSize === size.px}
											onclick={() => setAppearance({ fontSize: size.px })}
											class={segment(view.fontSize === size.px)}
										>
											{size.label()}
										</button>
									{/each}
								</div>
							</div>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<span class="text-sm font-medium">{m.settings_radius()}</span>
									<p class={hint}>{m.settings_radius_hint()}</p>
								</div>
								<div
									class="flex gap-0.5 rounded-lg border p-0.5"
									role="radiogroup"
									aria-label={m.settings_radius()}
								>
									{#each RADII as radius (radius.rem)}
										<button
											type="button"
											role="radio"
											aria-checked={view.radius === radius.rem}
											onclick={() => setAppearance({ radius: radius.rem })}
											class={segment(view.radius === radius.rem)}
										>
											{radius.label()}
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
								{m.settings_reset()}
							</Button>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="model">
					{@render header(m.settings_tab_model(), m.settings_model_description())}

					<div class="flex flex-col gap-6">
						<div class={group}>
							<div class={row}>
								<div class="flex flex-col gap-0.5">
									<Label for="model-enabled">{m.settings_model_enabled()}</Label>
									<p class={hint}>{m.settings_model_enabled_hint()}</p>
								</div>
								<Switch
									id="model-enabled"
									checked={view.modelEnabled}
									onCheckedChange={setModelEnabled}
								/>
							</div>
						</div>

						{#if view.modelEnabled}
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
												{choice.name()}
												{#if info?.activeVariant === choice.variant && view.modelPath === null}
													<Badge>{m.settings_model_in_use()}</Badge>
												{/if}
											</span>
											<span class={hint}>{choice.note()}</span>
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
											{m.settings_model_custom()}
											{#if view.modelPath !== null}<Badge>{m.settings_model_in_use()}</Badge>{/if}
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
											{m.settings_model_use()}
										</Button>
									</div>
								</div>
							</RadioGroup.Root>

							{#if model.state === 'downloading'}
								<div class="flex flex-col gap-2">
									<p class={hint}>
										{m.settings_model_downloading({ percent: model.percent ?? 0 })}
									</p>
									<Progress value={model.percent ?? 0} />
								</div>
							{/if}

							<p class={hint}>
								{#if info?.activePath}
									{#each parts(m.settings_model_active_path( { path: slot(info.activePath) } )) as part, i (i)}
										{#if i % 2}<span class="font-mono break-all">{part}</span>{:else}{part}{/if}
									{/each}
								{:else if view.modelPath !== null}
									<span class="text-amber-500">
										{m.settings_model_custom_missing()}
									</span>
								{:else}
									{m.settings_model_none()}
								{/if}
							</p>

							{#if info?.activeVariant && view.modelPath === null}
								<div class={group}>
									<div class={row}>
										<div class="flex flex-col gap-0.5">
											<span class="text-sm font-medium">{m.settings_updates()}</span>
											<p class={hint}>
												{#if updating !== null}
													{m.settings_update_downloading({ percent: updating })}
												{:else if update?.state === 'upToDate'}
													{m.settings_up_to_date({ revision: short(update.revision) })}
												{:else if update?.state === 'newer'}
													{m.settings_update_newer({
														latest: short(update.latest),
														installed: short(update.installed)
													})}
												{:else if update?.state === 'failed'}
													<span class="text-amber-500"
														>{m.settings_update_failed({ reason: update.reason })}</span
													>
												{:else}
													{m.settings_update_hint()}
												{/if}
											</p>
										</div>
										{#if update?.state === 'newer' && updating === null}
											<Button size="sm" onclick={applyUpdate}>
												<DownloadIcon />
												{m.settings_update()}
											</Button>
										{:else}
											<Button
												variant="secondary"
												size="sm"
												onclick={check}
												disabled={checking || updating !== null}
											>
												<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
												{checking ? m.settings_checking() : m.settings_check_updates()}
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
										<Label for="gpu">{m.settings_gpu()}</Label>
										<p class={hint}>
											{#if gpus.length === 0}
												{m.settings_gpu_none()}
											{:else}
												{m.settings_gpu_found({ devices: gpus.join(', ') })}
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
										<Label for="idle">{m.settings_idle()}</Label>
										<p class={hint}>{m.settings_idle_hint()}</p>
									</div>
									<div class="flex items-center gap-2">
										<Input
											id="idle"
											type="number"
											min="0"
											bind:value={draft.idleUnloadMinutes}
											class="w-20 text-right font-mono"
										/>
										<span class={hint}>{m.settings_idle_unit()}</span>
									</div>
								</div>
							</div>
						{/if}
					</div>
				</Tabs.Content>

				<Tabs.Content value="tags">
					{@render header(m.settings_tab_tags(), m.settings_aliases_description())}

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
							<Button onclick={saveAliases}>{m.settings_aliases_save()}</Button>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="index">
					{@render header(m.settings_tab_index(), m.settings_index_description())}

					<div class={group}>
						<div class={row}>
							<div class="flex flex-col gap-0.5">
								<span class="text-sm font-medium">{m.settings_rebuild_title()}</span>
								<p class={hint}>{m.settings_rebuild_hint()}</p>
							</div>
							<Button variant="secondary" size="sm" onclick={rebuild} disabled={rebuilding}>
								<RefreshCwIcon class={rebuilding ? 'animate-spin' : ''} />
								{rebuilding ? m.settings_rebuilding() : m.settings_rebuild()}
							</Button>
						</div>
					</div>

					<div
						class="mt-6 flex flex-col gap-3 rounded-lg border border-amber-500/40 bg-amber-500/5 px-4 py-3"
					>
						<div class="flex gap-3">
							<TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-amber-500" />
							<div class="flex flex-col gap-1">
								<span class="text-sm font-medium">{m.settings_regenerate_title()}</span>
								<p class={hint}>{m.settings_regenerate_hint()}</p>
								{#if !info?.activePath}
									<p class="text-xs text-amber-500">{m.settings_regenerate_needs_model()}</p>
								{/if}
							</div>
						</div>
						<div class="flex justify-end gap-2">
							{#if confirmRegenerate}
								<Button variant="ghost" size="sm" onclick={() => (confirmRegenerate = false)}>
									{m.common_cancel()}
								</Button>
								<Button variant="destructive" size="sm" onclick={regenerate}>
									{m.settings_regenerate_confirm()}
								</Button>
							{:else}
								<Button
									variant="outline"
									size="sm"
									onclick={() => (confirmRegenerate = true)}
									disabled={regenerating || !info?.activePath}
								>
									<SparklesIcon />
									{regenerating ? m.settings_regenerating() : m.settings_regenerate()}
								</Button>
							{/if}
						</div>
					</div>
				</Tabs.Content>
			{/if}
		</div>

		{#if dirty}
			<div class="flex items-center justify-between gap-4 border-t bg-muted/40 px-6 py-3">
				<p class={hint}>{m.settings_unsaved()}</p>
				<Button onclick={save}>{m.settings_save_changes()}</Button>
			</div>
		{/if}
	</div>
</Tabs.Root>
