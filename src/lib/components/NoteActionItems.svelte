<script lang="ts">
	import type { Note } from '#lib/api.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { aheadLabel } from '#lib/dates.js';
	import CalendarXIcon from '@lucide/svelte/icons/calendar-x';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RouteIcon from '@lucide/svelte/icons/route';
	import RouteOffIcon from '@lucide/svelte/icons/route-off';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		onedit,
		onsimilar,
		onpage,
		onmove,
		ondelete
	}: {
		/** The note or page the menu acts on. */
		note: Note;
		/** Edit it, with E as the shortcut. Left out where it is always being edited. */
		onedit?: () => void;
		/** List the notes closest in meaning. Left out without the embedding model. */
		onsimilar?: (note: Note) => void;
		/** Turn the note into a page. Left out for a page. */
		onpage?: (note: Note) => void;
		/** Move it to another space. Left out with one space. */
		onmove?: (note: Note) => void;
		/** Ask to delete; the view confirms. */
		ondelete: (note: Note) => void;
	} = $props();

	const shell = getShell();
	const inThread = $derived(shell.threads.threadOf(note.id) !== undefined);
	const keptOut = $derived(shell.threads.keptOut(note.id));
	const firstGroup = $derived(!!(onedit || onsimilar || onpage));
	const threadGroup = $derived(shell.canSimilar || inThread || keptOut);
	const placeGroup = $derived(!!(onmove || note.on));
</script>

<!-- The menu's items, inside its content: grouped by what each touches, the
     note, its thread, where it lives and the day it looks to, then deleting it. -->
{#if onedit}
	<DropdownMenu.Item onSelect={onedit}>
		<PencilIcon />{m.note_edit()}
		<DropdownMenu.Shortcut>E</DropdownMenu.Shortcut>
	</DropdownMenu.Item>
{/if}
{#if onsimilar}
	<DropdownMenu.Item onSelect={() => onsimilar(note)}>
		<WaypointsIcon />{m.note_similar()}
	</DropdownMenu.Item>
{/if}
{#if onpage}
	<DropdownMenu.Item onSelect={() => onpage(note)}>
		<FileTextIcon />{m.pages_turn_into()}
	</DropdownMenu.Item>
{/if}
{#if threadGroup}
	{#if firstGroup}<DropdownMenu.Separator />{/if}
	{#if shell.canSimilar}
		<DropdownMenu.Item onSelect={() => shell.threads.askThread(note)}>
			<RouteIcon />{inThread ? m.thread_move() : m.thread_add()}
		</DropdownMenu.Item>
	{/if}
	{#if inThread}
		<DropdownMenu.Item onSelect={() => void shell.threads.keepOut(note, true)}>
			<RouteOffIcon />{m.thread_leave()}
		</DropdownMenu.Item>
	{:else if keptOut}
		<DropdownMenu.Item onSelect={() => void shell.threads.keepOut(note, false)}>
			<RouteIcon />{m.thread_rejoin()}
		</DropdownMenu.Item>
	{/if}
{/if}
{#if placeGroup}
	{#if firstGroup || threadGroup}<DropdownMenu.Separator />{/if}
	{#if onmove}
		<DropdownMenu.Item onSelect={() => onmove(note)}>
			<FolderInputIcon />{m.move_to()}
		</DropdownMenu.Item>
	{/if}
	{#if note.on}
		{@const on = note.on}
		<DropdownMenu.Item onSelect={() => void shell.clearDayAhead(note)}>
			<CalendarXIcon />{m.day_ahead_clear({ date: aheadLabel(on) })}
		</DropdownMenu.Item>
	{/if}
{/if}
{#if firstGroup || threadGroup || placeGroup}<DropdownMenu.Separator />{/if}
<DropdownMenu.Item variant="destructive" onSelect={() => ondelete(note)}>
	<Trash2Icon />{m.common_delete()}
</DropdownMenu.Item>
