<script lang="ts">
	import { untrack } from 'svelte';
	import { listActivity, type ActivityEvent, type ActivityWhose } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import AppWindowIcon from '@lucide/svelte/icons/app-window';
	import BrainIcon from '@lucide/svelte/icons/brain';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import PaperclipIcon from '@lucide/svelte/icons/paperclip';
	import PuzzleIcon from '@lucide/svelte/icons/puzzle';
	import UserRoundIcon from '@lucide/svelte/icons/user-round';
	import { categoryLabel } from '$lib/categories';
	import { dayHeading } from '$lib/components/ViewHeader.svelte';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { pluginName } from '$lib/plugins/loader';
	import { getShell } from '$lib/shell.svelte';
	import Segmented from './Segmented.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint, section } from './styles';

	/** Settings > Activity (SPEC 4.11): who changed what in the open space. */
	let { settings }: { settings: SettingsState } = $props();

	const shell = getShell();

	/** How many more events each Show more reads. */
	const PAGE = 100;

	type Filter = ActivityWhose | 'all';
	const filters: { value: Filter; label: string }[] = [
		{ value: 'all', label: m.activity_filter_all() },
		{ value: 'user', label: m.activity_filter_user() },
		{ value: 'plugin', label: m.activity_filter_plugin() },
		{ value: 'model', label: m.activity_filter_model() },
		{ value: 'external', label: m.activity_filter_external() }
	];

	let whose = $state<Filter>('all');
	let events = $state<ActivityEvent[]>([]);
	let more = $state(false);
	let loading = $state(false);
	/** Counts the loads, so the answer to one overtaken by another is dropped. */
	let asked = 0;

	/**
	 * Read the newest `count` events. Show more reads them all again rather
	 * than the next ones, so events logged meanwhile shift nothing twice.
	 */
	async function load(filter: Filter, count = PAGE) {
		const ask = ++asked;
		loading = true;
		try {
			const page = await listActivity(0, count, filter === 'all' ? undefined : filter);
			if (ask !== asked) return;
			events = page.events;
			more = page.more;
		} catch (e) {
			if (ask === asked) settings.say(String(e), true);
		} finally {
			if (ask === asked) loading = false;
		}
	}

	// Read afresh whenever the tab shows, and when the filter changes.
	$effect(() => {
		if (settings.tab !== 'activity') return;
		const filter = whose;
		untrack(() => void load(filter));
	});

	/** The events by the day they happened, newest first, as the log has them. */
	const days = $derived.by(() => {
		const byDay: { date: string; events: ActivityEvent[] }[] = [];
		for (const event of events) {
			const date = event.at.slice(0, 10);
			const last = byDay.at(-1);
			if (last?.date === date) last.events.push(event);
			else byDay.push({ date, events: [event] });
		}
		return byDay;
	});

	/** Notes and pages deleted since, or turned into pages, which lead nowhere now. */
	const gone = $derived(
		new Set(
			events.flatMap((event) => {
				if (event.action === 'delete') return event.id ? [event.id] : [];
				const note = event.action === 'to-page' ? event.changes?.id?.[0] : undefined;
				return typeof note === 'string' ? [note] : [];
			})
		)
	);

	function actor(event: ActivityEvent) {
		if (event.actor === 'user') return { icon: UserRoundIcon, name: m.activity_actor_user() };
		if (event.actor === 'model') return { icon: BrainIcon, name: m.activity_actor_model() };
		if (event.actor === 'external')
			return { icon: AppWindowIcon, name: m.activity_actor_external() };
		const id = event.actor.startsWith('plugin:') ? event.actor.slice('plugin:'.length) : '';
		return { icon: PuzzleIcon, name: id ? pluginName(id) : event.actor };
	}

	const ACTIONS: Record<string, () => string> = {
		create: m.activity_create,
		edit: m.activity_edit,
		tick: m.activity_tick,
		label: m.activity_label,
		fail: m.activity_fail,
		rerun: m.activity_rerun,
		rename: m.activity_rename,
		move: m.activity_move,
		delete: m.activity_delete,
		'to-page': m.activity_to_page,
		attach: m.activity_attach,
		regenerate: m.activity_regenerate
	};

	/** An action a newer version logged reads as written. */
	const action = (event: ActivityEvent) => ACTIONS[event.action]?.() ?? event.action;

	function target(event: ActivityEvent): string {
		if (event.kind === 'space') return m.activity_every_note({ count: event.count ?? 0 });
		if (event.subject) return event.subject;
		if (event.kind === 'page') return m.pages_untitled();
		if (event.kind === 'attachment') return event.file?.split('/').pop() ?? '';
		return m.editor_untitled();
	}

	/** A day shorter than its heading: `22 Sep`, with the year when it is not this one. */
	function shortDay(date: string) {
		const day = new Date(`${date}T00:00:00`);
		const thisYear = day.getFullYear() === new Date().getFullYear();
		return day.toLocaleDateString(getLocale(), {
			day: 'numeric',
			month: 'short',
			...(thisYear ? {} : { year: 'numeric' })
		});
	}

	const words = (count: unknown) =>
		typeof count === 'number' ? m.pages_words({ count }) : undefined;
	const category = (name: unknown) =>
		typeof name === 'string' ? `#${categoryLabel(name)}` : m.activity_no_category();

	/** What changed, in a few words: the words, the category, the old title, the days. */
	function details(event: ActivityEvent): string {
		const changes = event.changes ?? {};
		const parts: string[] = [];
		if (changes.words) {
			const [before, after] = changes.words;
			if (typeof before === 'number' && typeof after === 'number') {
				if (before !== after) parts.push(`${before} → ${words(after)}`);
			} else {
				const either = words(after) ?? words(before);
				if (either) parts.push(either);
			}
		}
		if (changes.category) parts.push(changes.category.map(category).join(' → '));
		for (const old of [changes.title?.[0], changes.subject?.[0]]) {
			if (typeof old === 'string') parts.push(m.activity_was({ before: old }));
		}
		if (changes.date) {
			const [before, after] = changes.date;
			if (typeof before === 'string' && typeof after === 'string')
				parts.push(`${shortDay(before)} → ${shortDay(after)}`);
		}
		return parts.join(' · ');
	}

	const opens = (event: ActivityEvent) =>
		(event.kind === 'note' || event.kind === 'page') &&
		Boolean(event.id && event.date) &&
		!gone.has(event.id ?? '');

	function open(event: ActivityEvent) {
		if (!event.id || !event.date) return;
		shell.settingsOpen = false;
		void shell.openCited({
			id: event.id,
			date: event.date,
			kind: event.kind === 'page' ? 'page' : undefined
		});
	}
</script>

<div class="flex flex-col gap-6">
	<div class="self-start">
		<Segmented
			label={m.activity_filter_label()}
			options={filters}
			value={whose}
			onpick={(value) => (whose = value)}
		/>
	</div>

	{#if days.length === 0}
		{#if !loading}
			<p class={hint}>{m.activity_empty()}</p>
		{/if}
	{:else}
		{#each days as day (day.date)}
			<section class="flex flex-col gap-2">
				<h4 class={section}>{dayHeading(day.date)}</h4>
				<ul class={group}>
					{#each day.events as event, i (`${event.at}:${i}`)}
						{@const who = actor(event)}
						{@const detail = details(event)}
						<li class="flex items-center gap-3 px-4 py-2.5 text-sm">
							<time
								class="w-10 shrink-0 font-mono text-xs text-muted-foreground tabular-nums"
								datetime={event.at}
							>
								{event.at.slice(11, 16)}
							</time>
							<span class="flex w-36 shrink-0 items-center gap-1.5 text-muted-foreground">
								<who.icon class="size-3.5 shrink-0" />
								<span class="truncate" title={who.name}>{who.name}</span>
							</span>
							<span class="flex min-w-0 flex-1 items-center gap-1.5">
								<span class="shrink-0 font-medium">{action(event)}</span>
								{#if event.kind === 'page'}
									<FileTextIcon class="size-3.5 shrink-0 text-muted-foreground" />
								{:else if event.kind === 'attachment'}
									<PaperclipIcon class="size-3.5 shrink-0 text-muted-foreground" />
								{/if}
								{#if opens(event)}
									<button
										type="button"
										class="truncate text-left hover:underline"
										title={target(event)}
										onclick={() => open(event)}
									>
										{target(event)}
									</button>
								{:else}
									<span class="truncate text-muted-foreground" title={target(event)}>
										{target(event)}
									</span>
								{/if}
								{#if event.date && event.date !== day.date && !event.changes?.date}
									<span
										class="shrink-0 text-xs text-muted-foreground"
										title={m.activity_note_day()}
									>
										{shortDay(event.date)}
									</span>
								{/if}
							</span>
							{#if detail}
								<span
									class="max-w-[40%] shrink-0 truncate text-xs text-muted-foreground tabular-nums"
									title={detail}
								>
									{detail}
								</span>
							{/if}
						</li>
					{/each}
				</ul>
			</section>
		{/each}
		{#if more}
			<div class="flex justify-center">
				<Button
					variant="secondary"
					size="sm"
					disabled={loading}
					onclick={() => load(whose, events.length + PAGE)}
				>
					{m.activity_more()}
				</Button>
			</div>
		{/if}
	{/if}
</div>
