<script lang="ts">
	import type { Thread } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { cn } from '#lib/utils.js';

	let { thread, keepClass }: { thread: Thread; keepClass?: string } = $props();

	const shell = getShell();
</script>

<!-- A suggested thread dismissed or kept where it is listed. -->
<Button
	variant="ghost"
	size="sm"
	class="h-7 px-2 text-xs"
	onclick={() => void shell.threads.keepThread(thread.id, false)}
	aria-label={m.thread_dismiss_label({ name: shell.threads.nameOf(thread) })}
>
	{m.thread_dismiss()}
</Button>
<Button
	variant="outline"
	size="sm"
	class={cn('h-7 px-2 text-xs', keepClass)}
	onclick={() => void shell.threads.keepThread(thread.id, true)}
	aria-label={m.thread_keep_label({ name: shell.threads.nameOf(thread) })}
>
	{m.thread_keep()}
</Button>
