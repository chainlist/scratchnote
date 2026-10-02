<script lang="ts">
	import {
		createSpace,
		deleteSpace,
		openSpaceFolder,
		renameSpace,
		setActiveSpace,
		type SpaceSummary,
		type SpacesView
	} from '#lib/api.js';
	import { sidebarItem } from '#lib/components/sidebar.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import CheckIcon from '@lucide/svelte/icons/check';
	import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { parts, slot } from '#lib/i18n.svelte.js';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	let {
		view,
		chosen,
		onpick,
		hotkey,
		title,
		align = 'start',
		returnFocus,
		class: className
	}: {
		view: SpacesView | null;
		/** The space shown as the one in use: the open one unless another is given. */
		chosen?: string;
		/**
		 * Pick a space without opening it, as the capture window does for the
		 * one its note goes into. The list then only picks: no space is made,
		 * renamed or deleted from it.
		 */
		onpick?: (name: string) => void;
		/** The shortcut that picks the nth space, from 1, shown in its row in place of its count. */
		hotkey?: (n: number) => string;
		/** The button's tooltip, Switch space unless another is given. */
		title?: string;
		align?: 'start' | 'end';
		/** Where the focus goes as the list closes, rather than back to its button. */
		returnFocus?: () => void;
		/** The button's, merged over its own. */
		class?: string;
	} = $props();

	const current = $derived(chosen ?? view?.active);
	/** Making, renaming and deleting spaces, which a list that only picks leaves out. */
	const manage = $derived(onpick === undefined);

	let open = $state(false);
	let error = $state<string | null>(null);
	let newName = $state('');
	/** The space whose name is being edited, and the draft. */
	let renaming = $state<string | null>(null);
	let draft = $state('');
	/** The space waiting on a second click to be deleted. */
	let confirming = $state<string | null>(null);

	// Each time the list opens it starts clean.
	$effect(() => {
		if (!open) {
			error = null;
			renaming = null;
			confirming = null;
			newName = '';
		}
	});

	/** Runs an action; the page hears the outcome from `spaces-changed`. */
	async function act(action: () => Promise<unknown>): Promise<boolean> {
		try {
			await action();
			error = null;
			return true;
		} catch (e) {
			error = String(e);
			return false;
		}
	}

	async function pick(space: SpaceSummary) {
		if (onpick) {
			onpick(space.name);
			open = false;
			return;
		}
		if (renaming || space.name === view?.active) {
			open = false;
			return;
		}
		if (await act(() => setActiveSpace(space.name))) open = false;
	}

	async function create() {
		if (newName.trim() === '') return;
		if (await act(() => createSpace(newName))) open = false;
	}

	function startRename(space: SpaceSummary) {
		confirming = null;
		renaming = space.name;
		draft = space.name;
	}

	async function rename() {
		const from = renaming;
		if (!from) return;
		if (draft.trim() === '' || draft.trim() === from) {
			renaming = null;
			return;
		}
		if (await act(() => renameSpace(from, draft))) renaming = null;
	}

	async function remove(space: SpaceSummary) {
		if (confirming !== space.name) {
			renaming = null;
			confirming = space.name;
			return;
		}
		if (await act(() => deleteSpace(space.name))) confirming = null;
	}

	const icon = 'size-6 text-muted-foreground hover:text-foreground';
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<button
				{...props}
				type="button"
				title={title ?? m.spaces_switch()}
				class={cn(
					'-ml-2 flex min-w-0 items-center gap-1.5 rounded-md px-2 text-base leading-normal font-medium hover:bg-input/30',
					className
				)}
			>
				<span class="truncate">{current ?? 'Scratchnote'}</span>
				<ChevronsUpDownIcon class="size-3.5 shrink-0 text-muted-foreground" />
			</button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content
		class="max-h-(--bits-floating-available-height) w-64 overflow-y-auto bg-background p-1.5"
		{align}
		onCloseAutoFocus={returnFocus &&
			((event) => {
				event.preventDefault();
				returnFocus();
			})}
	>
		<p
			class="px-2 pt-1 pb-1.5 text-[0.6875rem] font-medium tracking-wide text-muted-foreground uppercase"
		>
			{m.spaces_heading()}
		</p>
		<ul class="flex flex-col gap-0.5">
			{#each view?.spaces ?? [] as space, i (space.name)}
				{@const on = space.name === current}
				<li class="group relative">
					{#if renaming === space.name}
						<form
							class="flex items-center gap-1"
							onsubmit={(event) => {
								event.preventDefault();
								void rename();
							}}
						>
							<Input
								bind:value={draft}
								autofocus
								spellcheck="false"
								aria-label={m.spaces_name_label()}
								class="h-8"
								onkeydown={(event) => {
									if (event.key === 'Escape') {
										event.preventDefault();
										event.stopPropagation();
										renaming = null;
									}
								}}
							/>
							<Button
								type="submit"
								variant="ghost"
								size="icon-sm"
								aria-label={m.spaces_save_name()}
							>
								<CheckIcon />
							</Button>
						</form>
					{:else if !manage}
						<button type="button" onclick={() => void pick(space)} class={sidebarItem(on)}>
							<span class="truncate">{space.name}</span>
							<kbd class="ml-auto font-mono text-xs text-muted-foreground">
								{hotkey && i < 9 ? hotkey(i + 1) : (space.notes ?? '')}
							</kbd>
						</button>
					{:else}
						<button type="button" onclick={() => void pick(space)} class="{sidebarItem(on)} pr-20">
							<span class="truncate">{space.name}</span>
							<span class="ml-auto font-mono text-xs text-muted-foreground group-hover:invisible">
								{space.notes ?? ''}
							</span>
						</button>
						<div
							class={[
								'absolute inset-y-0 right-1 flex items-center gap-0.5',
								confirming === space.name
									? ''
									: 'invisible group-focus-within:visible group-hover:visible'
							]}
						>
							{#if confirming === space.name}
								<Button
									variant="destructive"
									size="sm"
									class="h-6 px-2 text-xs"
									onclick={() => void remove(space)}
								>
									{m.common_delete()}
								</Button>
							{:else}
								<button
									type="button"
									class={icon}
									onclick={() => void act(() => openSpaceFolder(space.name))}
									aria-label={m.spaces_open_folder_label({ name: space.name })}
									title={m.spaces_open_folder()}
								>
									<FolderOpenIcon class="mx-auto size-3.5" />
								</button>
								<button
									type="button"
									class={icon}
									onclick={() => startRename(space)}
									aria-label={m.spaces_rename_label({ name: space.name })}
									title={m.spaces_rename()}
								>
									<PencilIcon class="mx-auto size-3.5" />
								</button>
								{#if (view?.spaces.length ?? 0) > 1}
									<button
										type="button"
										class={icon}
										onclick={() => void remove(space)}
										aria-label={m.spaces_delete_label({ name: space.name })}
										title={m.common_delete()}
									>
										<Trash2Icon class="mx-auto size-3.5" />
									</button>
								{/if}
							{/if}
						</div>
					{/if}
				</li>
			{/each}
		</ul>

		{#if confirming}
			<p class="px-2 pt-2 text-xs text-muted-foreground">
				{#each parts(m.spaces_trash_hint( { trash: slot('.scratchnote/trash'), spaces: slot('spaces') } )) as part, i (i)}
					{#if i % 2}<span class="font-mono">{part}</span>{:else}{part}{/if}
				{/each}
			</p>
		{/if}

		{#if manage}
			<form
				class="mt-1.5 flex items-center gap-1 border-t pt-1.5"
				onsubmit={(event) => {
					event.preventDefault();
					void create();
				}}
			>
				<Input
					bind:value={newName}
					placeholder={m.spaces_new_placeholder()}
					spellcheck="false"
					class="h-8"
				/>
				<Button
					type="submit"
					variant="ghost"
					size="icon-sm"
					aria-label={m.spaces_create()}
					disabled={newName.trim() === ''}
				>
					<PlusIcon />
				</Button>
			</form>
		{/if}

		{#if error}
			<p class="px-2 pt-1.5 text-xs text-red-400">{error}</p>
		{/if}
	</Popover.Content>
</Popover.Root>
