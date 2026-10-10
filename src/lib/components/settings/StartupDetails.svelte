<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { pluginName } from '#lib/plugins/loader.js';
	import { duration, type StartupRow, type StartupTimes } from '#lib/startup.js';

	/** This window's startup, step by step, as the console logs it. */
	let { open = $bindable(false), times }: { open: boolean; times: StartupTimes } = $props();

	const NAMES = {
		app: m.settings_startup_app,
		plugins: m.settings_startup_plugins,
		list: m.settings_startup_list,
		listeners: m.settings_startup_listeners,
		view: m.settings_startup_view
	};

	const name = (row: StartupRow) =>
		row.step === 'detail'
			? row.label
			: row.step === 'plugin'
				? pluginName(row.plugin.id)
				: NAMES[row.step]();
	const time = (ms: number) => duration(ms, getLocale());
	const percent = (share: number) =>
		new Intl.NumberFormat(getLocale(), { style: 'percent' }).format(share);
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[calc(100%-2rem)] overflow-y-auto sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>{m.settings_startup_title()}</Dialog.Title>
			<Dialog.Description>{m.settings_startup_description()}</Dialog.Description>
		</Dialog.Header>
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>{m.settings_startup_step()}</Table.Head>
					<Table.Head class="text-right">{m.settings_startup_time()}</Table.Head>
					<Table.Head class="w-28">{m.settings_startup_share()}</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each times.rows as row, i (i)}
					{@const share = row.ms / times.total}
					<Table.Row>
						<Table.Cell class={row.nested ? 'pl-6 text-muted-foreground' : 'font-medium'}>
							<div class="flex items-center gap-2">
								<span class="truncate">{name(row)}</span>
								{#if row.plugin}
									<Badge variant="secondary">
										{row.plugin.core ? m.settings_startup_core() : m.settings_startup_community()}
									</Badge>
								{/if}
							</div>
							{#if row.plugin?.error}
								<p class="text-xs whitespace-normal text-destructive">
									{m.plugins_failed_because({ error: row.plugin.error })}
								</p>
							{/if}
						</Table.Cell>
						<Table.Cell class="text-right tabular-nums">{time(row.ms)}</Table.Cell>
						<Table.Cell title={percent(share)}>
							<!-- The time is said in the cell before; the bar only shows it. -->
							<div aria-hidden="true" class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
								<div
									class="h-full rounded-full {row.nested ? 'bg-primary/50' : 'bg-primary'}"
									style:width="{share * 100}%"
								></div>
							</div>
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
			<Table.Footer>
				<Table.Row>
					<Table.Cell class="font-medium">{m.settings_startup_all()}</Table.Cell>
					<Table.Cell class="text-right font-medium tabular-nums">{time(times.total)}</Table.Cell>
					<Table.Cell></Table.Cell>
				</Table.Row>
			</Table.Footer>
		</Table.Root>
	</Dialog.Content>
</Dialog.Root>
