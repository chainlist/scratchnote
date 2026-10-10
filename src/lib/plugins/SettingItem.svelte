<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { hint, section } from '#lib/components/settings/styles.js';
	import { call, type SettingModel } from './setting-model';

	/** One `Setting` of a plugin's tab; the builder changes the model it draws. */
	let { model }: { model: SettingModel } = $props();
</script>

<div class="flex min-w-0 flex-col gap-0.5">
	{#if model.heading}
		<h4 class={section}>{model.name}</h4>
	{:else}
		<span class="text-sm font-medium">{model.name}</span>
	{/if}
	{#if model.desc}<p class={hint}>{model.desc}</p>{/if}
</div>
<div class="flex shrink-0 items-center gap-2">
	{#each model.controls as control, i (i)}
		{#if control.kind === 'toggle'}
			<Switch
				checked={control.value}
				disabled={control.disabled}
				aria-label={model.name}
				onCheckedChange={(value) => {
					control.value = value;
					call(control.change, value);
				}}
			/>
		{:else if control.kind === 'text'}
			<Input
				value={control.value}
				placeholder={control.placeholder}
				disabled={control.disabled}
				aria-label={model.name}
				class="h-7 w-48"
				oninput={(event) => {
					control.value = event.currentTarget.value;
					call(control.change, control.value);
				}}
			/>
		{:else if control.kind === 'dropdown'}
			<Select.Root
				type="single"
				value={control.value}
				disabled={control.disabled}
				onValueChange={(value) => {
					control.value = value;
					call(control.change, value);
				}}
			>
				<Select.Trigger size="sm" class="w-44" aria-label={model.name}>
					{control.options.find((option) => option.value === control.value)?.label ?? ''}
				</Select.Trigger>
				<Select.Content>
					{#each control.options as option (option.value)}
						<Select.Item value={option.value} label={option.label} />
					{/each}
				</Select.Content>
			</Select.Root>
		{:else}
			<Button
				size="sm"
				variant={control.variant}
				disabled={control.disabled}
				onclick={() => call(control.click, undefined)}
			>
				{control.text}
			</Button>
		{/if}
	{/each}
</div>
