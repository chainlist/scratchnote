<script lang="ts">
	import { openAttachment, openLink } from '$lib/api';
	import { attachmentUrl } from '$lib/attachments.svelte';
	import { cardName, fileName, fileType, renderLines } from '$lib/markdown';

	let {
		text,
		links = true,
		class: className = ''
	}: {
		/** A note body, in markdown. */
		text: string;
		/** Off for a preview: its links are drawn but neither open nor take focus, and an attachment shows as its name. */
		links?: boolean;
		class?: string;
	} = $props();

	const lines = $derived(renderLines(text));

	function open(event: MouseEvent, href: string) {
		// The page would otherwise navigate away. The second click of a double
		// click would open the link twice.
		event.preventDefault();
		if (event.detail < 2) void openLink(href);
	}

	/** In its own app, as the file manager would: a PDF in the PDF viewer. */
	function openFile(event: MouseEvent, path: string) {
		if (event.detail < 2) void openAttachment(path);
	}
</script>

<!-- One block per line of the note, laid out as the editor lays out its
     lines, so a note reads the same as it was written. -->
<div class={className}>
	{#each lines as line, i (i)}
		<div class="md-line {line.class}" style={line.style}>
			{#each line.parts as part, k (k)}{#if 'bullet' in part}<span class="md-bullet"
					></span>{:else if 'attachment' in part}{@const path = part.attachment}{@const name =
						part.name || fileName(path)}{#if !links}<span class="md-chip">{name}</span
						>{:else if part.image}<button
							type="button"
							class="md-image-button"
							onclick={(event) => openFile(event, path)}
							><img src={attachmentUrl(path)} alt={name} class="md-image" /></button
						>{:else}<button
							type="button"
							class="md-file"
							title={name}
							onclick={(event) => openFile(event, path)}
							><span class="md-file-type">{fileType(path)}</span><span class="md-file-name"
								>{cardName(name, path)}</span
							></button
						>{/if}{:else if part.href && links}{@const href = part.href}<a
						{href}
						class={part.class}
						onclick={(event) => open(event, href)}>{part.text}</a
					>{:else if part.class}<span class={part.class}>{part.text}</span
					>{:else}{part.text}{/if}{:else}<br />{/each}
		</div>
	{/each}
</div>
