<script lang="ts">
	import ThreadView from '#lib/components/ThreadView.svelte';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadName } from '#lib/threads.js';

	let { data, params } = $props();

	const shell = getShell();
	const thread = $derived(data.found?.thread ?? null);
</script>

<!-- A thread in the timeline's place, under its title (SPEC 6.4). The view
     is keyed by the thread, so another one starts with nothing chosen. -->
<View
	back={shell.back}
	title={thread ? threadName(thread, data.found?.notes[0]) : m.thread_untitled()}
	pin={thread ? { kind: 'thread', target: thread.id } : undefined}
	detail={thread
		? (thread.kept ? '' : `${m.threads_suggested()} · `) +
			m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })
		: undefined}
	tentative={thread ? !thread.kept : false}
	key={params.id}
>
	<ThreadView found={data.found} />
</View>
