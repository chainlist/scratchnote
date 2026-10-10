import { cn } from '#lib/utils.js';

/**
 * A sidebar row, styled like the settings dialog's vertical tabs so the app
 * sidebar and the settings sidebar read as one design.
 */
export const sidebarItem = (active: boolean) =>
	cn(
		'flex h-8 w-full items-center gap-2 rounded-md border border-transparent px-2 text-left text-sm font-medium transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-4',
		active
			? 'border-input bg-input/30 text-foreground'
			: 'text-muted-foreground hover:text-foreground'
	);
