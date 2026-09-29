import { mount, unmount } from 'svelte';
import Markdown from '$lib/components/Markdown.svelte';
import type { RenderOptions } from './api';

/** Draw markdown with the card's own component, into a plugin's element. */
export function render(el: HTMLElement, text: string, options: RenderOptions = {}) {
	const props = $state({
		text,
		links: !options.preview,
		onchange: options.onchange,
		class: options.class ?? ''
	});
	const drawn = mount(Markdown, { target: el, props });
	return {
		update(next: string) {
			props.text = next;
		},
		destroy() {
			void unmount(drawn);
		}
	};
}
