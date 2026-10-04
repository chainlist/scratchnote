<script lang="ts">
	import type { ThreadOrder } from '#lib/api.js';
	import { m } from '#lib/paraglide/messages.js';
	import Segmented from './Segmented.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	const orders: { value: ThreadOrder; label: () => string }[] = [
		{ value: 'oldest', label: m.settings_thread_order_oldest },
		{ value: 'newest', label: m.settings_thread_order_newest }
	];
</script>

{#if settings.view}
	{@const view = settings.view}
	<div class={group}>
		<SettingRow label={m.settings_thread_order()} hint={m.settings_thread_order_hint()}>
			<Segmented
				label={m.settings_thread_order()}
				options={orders.map((order) => ({ value: order.value, label: order.label() }))}
				value={view.threadOrder}
				onpick={(threadOrder) => settings.apply({ threadOrder })}
			/>
		</SettingRow>
	</div>
{/if}
