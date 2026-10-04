<script lang="ts">
	import { resolve } from '$app/paths';
	import type { ThreadCard } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();
	const suggested = $derived(data.threads.filter((thread) => !thread.kept));
	const yours = $derived(data.threads.filter((thread) => thread.kept));

	function firstLine(body: string) {
		return body.split('\n').find((line) => line.trim()) ?? '';
	}

	/** A thread's title, else its first note's first line. */
	function name(thread: ThreadCard) {
		const first = thread.first[0];
		return thread.title ?? (first ? (first.subject ?? firstLine(first.body)) : m.thread_untitled());
	}
</script>

<!-- The space's threads: those only suggested, to keep or dismiss, then the
     user's, the one written in last first (SPEC 6.4). -->
<View back={shell.back} title={m.threads_all()}>
	{#if data.threads.length === 0}
		<p class="text-base text-neutral-600">{m.threads_none()}</p>
	{/if}
	{#if suggested.length}
		<section class="mb-10">
			<h2 class="border-b border-neutral-800 pb-2 text-sm font-medium text-neutral-400">
				{m.threads_suggested()}
			</h2>
			<p class="mt-3 mb-4 text-sm text-neutral-500">{m.threads_suggested_hint()}</p>
			<ul class="flex flex-col gap-3">
				{#each suggested as thread (thread.id)}
					<li class="rounded-lg border border-neutral-800 px-4 py-3">
						<a
							href={resolve(`thread/${thread.id}/`)}
							class="block rounded text-sm text-neutral-300 hover:text-neutral-100"
						>
							<span class="text-xs text-neutral-500">
								{m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })}
							</span>
							{#each thread.first as note (note.id)}
								<Markdown text={note.body} links={false} class="mt-1 line-clamp-1" />
							{/each}
						</a>
						<div class="mt-3 flex justify-end gap-2">
							<Button
								variant="ghost"
								size="sm"
								onclick={() => void shell.keepThread(thread.id, false)}
							>
								{m.thread_dismiss()}
							</Button>
							<Button
								variant="outline"
								size="sm"
								onclick={() => void shell.keepThread(thread.id, true)}
							>
								{m.thread_keep()}
							</Button>
						</div>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
	{#if yours.length}
		<section>
			<h2 class="border-b border-neutral-800 pb-2 text-sm font-medium text-neutral-400">
				{m.threads_yours()}
			</h2>
			<ul>
				{#each yours as thread (thread.id)}
					<li>
						<a
							href={resolve(`thread/${thread.id}/`)}
							class="-mx-2 flex items-baseline justify-between gap-4 rounded px-2 py-2 hover:bg-neutral-900"
						>
							<span class="truncate text-neutral-200">{name(thread)}</span>
							<span class="shrink-0 text-xs text-neutral-500">
								{m.thread_last({ count: thread.notes.length, date: shortDay(thread.until) })}
							</span>
						</a>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
</View>
