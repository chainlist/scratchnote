<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { EMBEDDING_SIZE, restartApp } from '#lib/api.js';
	import WelcomeStep from './WelcomeStep.svelte';
	import AppearanceTab from '#lib/components/settings/AppearanceTab.svelte';
	import EmbeddingModel from '#lib/components/settings/EmbeddingModel.svelte';
	import FolderPicker from '#lib/components/settings/FolderPicker.svelte';
	import HotkeyInput from '#lib/components/settings/HotkeyInput.svelte';
	import { SettingsState } from '#lib/components/settings/state.svelte.js';
	import { hint } from '#lib/components/settings/styles.js';
	import WindowControls from '#lib/components/layout/WindowControls.svelte';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CheckIcon from '@lucide/svelte/icons/check';
	import { RadioGroup } from 'bits-ui';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import LanguagesIcon from '@lucide/svelte/icons/languages';
	import PaletteIcon from '@lucide/svelte/icons/palette';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import { languageName, LANGUAGES } from '#lib/i18n.svelte.js';
	import { clearResumeStep, resumeStep, setResumeStep } from '#lib/onboarding.js';
	import { m } from '#lib/paraglide/messages.js';
	import { android, mac } from '#lib/platform.js';
	import { app } from '#lib/plugins/app.js';

	/**
	 * The first-run walkthrough. The main page sends a fresh install here;
	 * every choice is saved as it is made, so leaving part way loses nothing.
	 * The folder comes before the search model, which is downloaded into it.
	 * Android has no capture hotkey, so it skips that step.
	 */
	const STEPS = [
		{
			id: 'language',
			icon: LanguagesIcon,
			title: m.onboarding_language_title,
			body: m.onboarding_language_body
		},
		{
			id: 'welcome',
			icon: SparklesIcon,
			title: m.onboarding_welcome_title,
			body: m.onboarding_welcome_body
		},
		{
			id: 'folder',
			icon: FolderIcon,
			title: m.onboarding_folder_title,
			body: m.onboarding_folder_body
		},
		{
			id: 'hotkey',
			icon: KeyboardIcon,
			title: m.onboarding_hotkey_title,
			body: m.onboarding_hotkey_body
		},
		{
			id: 'model',
			icon: DownloadIcon,
			title: m.onboarding_model_title,
			body: () => m.onboarding_model_body({ size: EMBEDDING_SIZE })
		},
		{
			id: 'appearance',
			icon: PaletteIcon,
			title: m.onboarding_appearance_title,
			body: m.onboarding_appearance_body
		}
	].filter((s) => !(android && s.id === 'hotkey'));

	const settings = new SettingsState();
	let index = $state(0);
	const step = $derived(STEPS[index]);
	const last = $derived(index === STEPS.length - 1);
	let working = $state(false);

	const view = $derived(settings.view);
	/** A folder other than the one this run uses, which takes a restart. */
	const restartNeeded = $derived(
		step.id === 'folder' && view !== null && settings.draft.root.trim() !== view.activeRoot
	);

	onMount(() => {
		const stop = settings.start();
		const saved = resumeStep();
		if (saved !== null && saved < STEPS.length) index = saved;
		return stop;
	});

	function go(to: number) {
		settings.message = null;
		index = to;
		setResumeStep(to);
		document.querySelector('main')?.scrollTo({ top: 0 });
	}

	/** Saves a new folder; false while the app restarts into it or on an error. */
	async function saveFolder(): Promise<boolean> {
		const root = settings.draft.root.trim();
		if (root !== view!.root && !(await settings.apply({ root }))) return false;
		if (settings.view!.root === settings.view!.activeRoot) return true;
		go(index + 1);
		await restartApp();
		return false;
	}

	async function next() {
		working = true;
		try {
			if (step.id === 'folder' && !(await saveFolder())) return;
			if (!last) return go(index + 1);
			// The walkthrough is a new install's introduction, so its release
			// notes are not shown after it.
			if (!(await settings.apply({ onboarded: true, lastSeenVersion: app.version }))) return;
			clearResumeStep();
			// In place of the walkthrough, so Android's back button cannot
			// return to it.
			await goto(resolve('/(app)'), { replace: true });
		} finally {
			working = false;
		}
	}
</script>

<div class="flex h-screen flex-col bg-background text-foreground">
	<!-- macOS keeps its native traffic lights over the top left corner. -->
	<header
		data-tauri-drag-region="deep"
		class={['flex h-12 shrink-0 items-center gap-3 pr-2', mac ? 'pl-20' : 'pl-4']}
	>
		<span class="text-sm font-medium">Scratchnote</span>
		<div class="ml-auto">
			{#if !mac && !android}<WindowControls />{/if}
		</div>
	</header>

	<main class="min-h-0 flex-1 overflow-y-auto px-6">
		<div class="mx-auto flex max-w-xl flex-col gap-6 pt-6 pb-10">
			<div class="flex gap-1" aria-hidden="true">
				{#each STEPS as s, i (s.id)}
					<div
						class="h-1 flex-1 rounded-full transition-colors {i <= index
							? 'bg-primary'
							: 'bg-muted'}"
					></div>
				{/each}
			</div>

			<div class="flex flex-col gap-2">
				<step.icon class="size-6 text-primary" />
				<h1 id="onboarding-step" class="text-xl font-semibold">{step.title()}</h1>
				<p class="text-sm text-muted-foreground">{step.body()}</p>
			</div>

			{#if settings.message?.error}
				<Alert.Root variant="destructive">
					<CircleAlertIcon />
					<Alert.Description>{settings.message.text}</Alert.Description>
				</Alert.Root>
			{/if}

			{#if view}
				{#if step.id === 'language'}
					<!-- One stop for Tab; the arrows move between the languages. -->
					<RadioGroup.Root
						value={view.language}
						onValueChange={(picked) => {
							const language = LANGUAGES.find((l) => l === picked);
							if (language) void settings.apply({ language });
						}}
						aria-labelledby="onboarding-step"
						class="grid grid-cols-2 gap-2"
					>
						{#each LANGUAGES as language (language)}
							{@const selected = view.language === language}
							<RadioGroup.Item
								value={language}
								class="flex items-center justify-between rounded-lg border bg-card px-4 py-3 text-left text-sm transition-colors outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 {selected
									? 'border-primary bg-primary/5'
									: 'hover:bg-accent'}"
							>
								{languageName(language)}
								{#if selected}<CheckIcon class="size-4 text-primary" />{/if}
							</RadioGroup.Item>
						{/each}
					</RadioGroup.Root>
				{:else if step.id === 'welcome'}
					<WelcomeStep />
				{:else if step.id === 'folder'}
					<div class="flex flex-col gap-2">
						<FolderPicker
							value={settings.draft.root}
							appStorage={view.defaultRoot}
							onchange={(root) => (settings.draft.root = root)}
							onerror={(message) => settings.say(message, true)}
						/>
						{#if restartNeeded}
							<p class="text-xs text-warning">{m.onboarding_folder_restart()}</p>
						{/if}
					</div>
				{:else if step.id === 'hotkey'}
					<div class="flex flex-col items-start gap-3">
						<!-- Named by the step's title, the hint read after it. -->
						<HotkeyInput
							value={view.captureHotkey}
							onchange={(captureHotkey) => settings.apply({ captureHotkey })}
							aria-labelledby="onboarding-step"
							aria-describedby="onboarding-hotkey-hint"
							class="h-11 w-72 text-base"
						/>
						<p id="onboarding-hotkey-hint" class={hint}>{m.onboarding_hotkey_try()}</p>
					</div>
				{:else if step.id === 'model'}
					<EmbeddingModel {settings} />
				{:else if step.id === 'appearance'}
					<AppearanceTab {settings} withLanguage={false} />
				{/if}
			{/if}
		</div>
	</main>

	<footer class="shrink-0 border-t bg-muted/40 px-6 py-3">
		<div class="mx-auto flex max-w-xl items-center gap-4">
			{#if index > 0}
				<Button variant="ghost" onclick={() => go(index - 1)} disabled={working}>
					{m.onboarding_back()}
				</Button>
			{/if}
			<p class="{hint} ml-auto">
				{m.onboarding_step({ current: index + 1, total: STEPS.length })}
			</p>
			<Button onclick={next} disabled={working || !view}>
				{#if restartNeeded}
					{m.onboarding_folder_restart_button()}
				{:else if last}
					{m.onboarding_finish()}
				{:else}
					{m.onboarding_next()}
				{/if}
			</Button>
		</div>
	</footer>
</div>
