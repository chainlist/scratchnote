<script lang="ts" module>
	import { tv, type VariantProps } from 'tailwind-variants';

	export const tabsListVariants = tv({
		base: 'relative rounded-lg p-[3px] group-data-horizontal/tabs:h-8 data-[variant=line]:rounded-none group/tabs-list inline-flex w-fit items-center justify-center text-muted-foreground group-data-vertical/tabs:h-fit group-data-vertical/tabs:flex-col',
		variants: {
			variant: {
				default: 'bg-muted',
				line: 'gap-1 bg-transparent'
			}
		},
		defaultVariants: {
			variant: 'default'
		}
	});

	export type TabsListVariant = VariantProps<typeof tabsListVariants>['variant'];
</script>

<script lang="ts">
	import { Tabs as TabsPrimitive } from 'bits-ui';
	import { cn } from '#lib/utils.js';
	import { follow } from '#lib/helpers/motion.js';

	let {
		ref = $bindable(null),
		variant = 'default',
		class: className,
		children,
		...restProps
	}: TabsPrimitive.ListProps & {
		variant?: TabsListVariant;
	} = $props();
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	data-variant={variant}
	class={cn(tabsListVariants({ variant }), className)}
	{...restProps}
>
	<!-- The chosen tab's pill, or its underline on a line list, gliding to
	     the tab picked; the tab itself only changes its text. -->
	<span
		aria-hidden="true"
		class={cn(
			'glide-mark',
			variant === 'line'
				? 'h-0.5 bg-foreground'
				: 'rounded-md bg-background shadow-sm dark:border dark:border-input dark:bg-input/30'
		)}
		style={variant === 'line'
			? 'translate: var(--mark-x) calc(var(--mark-y) + var(--mark-h) + var(--tabs-mark-drop, 3px))'
			: undefined}
		{@attach follow('[data-slot="tabs-trigger"][data-state="active"]')}
	></span>
	{@render children?.()}
</TabsPrimitive.List>
