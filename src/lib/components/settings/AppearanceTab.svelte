<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import CheckIcon from '@lucide/svelte/icons/check';
	import { RadioGroup } from 'bits-ui';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import {
		ACCENTS,
		DEFAULT_APPEARANCE,
		FONT_SIZES,
		FONTS,
		RADII,
		THEMES
	} from '#lib/app/appearance.js';
	import { languageName, LANGUAGES, type Language } from '#lib/app/i18n.svelte.js';
	import { m } from '#lib/paraglide/messages.js';
	import Segmented from './Segmented.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group } from './styles';

	let {
		settings,
		withLanguage = true
	}: {
		settings: SettingsState;
		/** Off where the language was picked already, as in the onboarding. */
		withLanguage?: boolean;
	} = $props();

	// Tabs are only drawn once the settings have loaded. Every choice here
	// applies at once: each window repaints on settings-changed.
	const view = $derived(settings.view!);

	const isDefault = $derived(
		view.accentColor === DEFAULT_APPEARANCE.accentColor &&
			view.fontFamily === DEFAULT_APPEARANCE.fontFamily &&
			view.fontSize === DEFAULT_APPEARANCE.fontSize &&
			view.radius === DEFAULT_APPEARANCE.radius &&
			view.theme === DEFAULT_APPEARANCE.theme &&
			view.reduceMotion === DEFAULT_APPEARANCE.reduceMotion
	);
</script>

<div class="flex flex-col gap-4">
	<div class={group}>
		{#if withLanguage}
			<SettingRow label={m.settings_language()} hint={m.settings_language_hint()}>
				<Select.Root
					type="single"
					value={view.language}
					onValueChange={(language) => settings.apply({ language: language as Language })}
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
			</SettingRow>
		{/if}
		<SettingRow label={m.settings_theme()} hint={m.settings_theme_hint()}>
			<Segmented
				label={m.settings_theme()}
				options={THEMES.map((theme) => ({ value: theme.name, label: theme.label() }))}
				value={view.theme}
				onpick={(theme) => settings.apply({ theme })}
			/>
		</SettingRow>
		<SettingRow label={m.settings_accent()} hint={m.settings_accent_hint()}>
			<!-- One stop for Tab; the arrows move between the colours. -->
			<RadioGroup.Root
				value={view.accentColor}
				onValueChange={(accentColor) => settings.apply({ accentColor })}
				orientation="horizontal"
				aria-label={m.settings_accent()}
				class="flex gap-2"
			>
				{#each ACCENTS as accent (accent.name)}
					{@const selected = view.accentColor === accent.name}
					<RadioGroup.Item
						value={accent.name}
						aria-label={accent.label()}
						title={accent.label()}
						class="flex size-7 cursor-pointer items-center justify-center rounded-full bg-(--swatch-light) ring-offset-2 ring-offset-card transition outline-none focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-ring focus-visible:outline-solid dark:bg-(--swatch-dark) {selected
							? 'ring-2 ring-foreground'
							: 'hover:ring-2 hover:ring-border'}"
						style="--swatch-light: {accent.light.swatch}; --swatch-dark: {accent.dark.swatch}"
					>
						{#if selected}
							<CheckIcon
								class="size-4 text-(--on-light) dark:text-(--on-dark)"
								style="--on-light: {accent.light.foreground}; --on-dark: {accent.dark.foreground}"
							/>
						{/if}
					</RadioGroup.Item>
				{/each}
			</RadioGroup.Root>
		</SettingRow>
		<SettingRow label={m.settings_font()} hint={m.settings_font_hint()}>
			<Select.Root
				type="single"
				value={view.fontFamily}
				onValueChange={(fontFamily) => settings.apply({ fontFamily })}
			>
				<Select.Trigger size="sm" class="w-48" aria-label={m.settings_font()}>
					{(FONTS.find((f) => f.name === view.fontFamily) ?? FONTS[0]).label()}
				</Select.Trigger>
				<Select.Content>
					{#each FONTS as font (font.name)}
						<Select.Item value={font.name} label={font.label()}>
							<span style="font-family: {font.family}">{font.label()}</span>
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</SettingRow>
		<SettingRow label={m.settings_text_size()} hint={m.settings_text_size_hint()}>
			<Segmented
				label={m.settings_text_size()}
				options={FONT_SIZES.map((size) => ({ value: size.px, label: size.label() }))}
				value={view.fontSize}
				onpick={(fontSize) => settings.apply({ fontSize })}
			/>
		</SettingRow>
		<SettingRow label={m.settings_radius()} hint={m.settings_radius_hint()}>
			<Segmented
				label={m.settings_radius()}
				options={RADII.map((radius) => ({ value: radius.rem, label: radius.label() }))}
				value={view.radius}
				onpick={(radius) => settings.apply({ radius })}
			/>
		</SettingRow>
		<SettingRow
			id="reduce-motion"
			label={m.settings_reduce_motion()}
			hint={m.settings_reduce_motion_hint()}
		>
			<Switch
				id="reduce-motion"
				checked={view.reduceMotion}
				onCheckedChange={(reduceMotion) => settings.apply({ reduceMotion })}
			/>
		</SettingRow>
	</div>
	<div class="flex justify-end">
		<Button
			variant="outline"
			size="sm"
			onclick={() => settings.apply(DEFAULT_APPEARANCE)}
			disabled={isDefault}
		>
			{m.settings_reset()}
		</Button>
	</div>
</div>
