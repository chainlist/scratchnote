<script lang="ts">
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import LayersIcon from '@lucide/svelte/icons/layers';
	import MessageCircleIcon from '@lucide/svelte/icons/message-circle';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import TagIcon from '@lucide/svelte/icons/tag';
	import { m } from '#lib/paraglide/messages.js';

	const features = [
		{
			icon: KeyboardIcon,
			title: m.onboarding_feature_capture_title,
			body: m.onboarding_feature_capture_body
		},
		{
			icon: TagIcon,
			title: m.onboarding_feature_labels_title,
			body: m.onboarding_feature_labels_body
		},
		{
			icon: MessageCircleIcon,
			title: m.onboarding_feature_chat_title,
			body: m.onboarding_feature_chat_body
		},
		{
			icon: LayersIcon,
			title: m.onboarding_feature_spaces_title,
			body: m.onboarding_feature_spaces_body
		}
	];
</script>

<!-- The promise the rest rests on, so it comes first and stands out. -->
<div class="flex gap-4 rounded-lg border border-primary bg-primary/5 p-4">
	<ShieldCheckIcon class="mt-0.5 size-6 shrink-0 text-primary" />
	<div class="flex flex-col gap-1">
		<p class="font-semibold">{m.onboarding_local_title()}</p>
		<p class="text-sm text-muted-foreground">{m.onboarding_local_body()}</p>
	</div>
</div>

<div class="grid gap-3 sm:grid-cols-2">
	{#each features as feature (feature.title)}
		<div class="flex flex-col gap-2 rounded-lg border bg-card p-4">
			<feature.icon class="size-5 text-primary" />
			<p class="text-sm font-medium">{feature.title()}</p>
			<p class="text-sm text-muted-foreground">{feature.body()}</p>
		</div>
	{/each}
</div>
