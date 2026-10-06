<script lang="ts">
	import { Tabs as TabsPrimitive } from 'bits-ui';
	import { cn } from '#lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: TabsPrimitive.ContentProps = $props();
</script>

<!-- A tab not chosen draws nothing. bits-ui would draw it and only hide
     it, which costs every hidden tab's content up front. -->
<TabsPrimitive.Content
	bind:ref
	data-slot="tabs-content"
	class={cn('flex-1 text-sm outline-none', className)}
	{...restProps}
>
	{#snippet child({ props })}
		<div {...props}>
			{#if !props.hidden}{@render children?.()}{/if}
		</div>
	{/snippet}
</TabsPrimitive.Content>
