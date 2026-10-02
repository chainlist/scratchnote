import type { ClientInit, HandleClientError } from '@sveltejs/kit/hooks';
import { startPlugins } from '#lib/plugins/loader.js';

// The plugins load before the first view, so the first card is drawn with
// their syntax and the first view finds their pages (SPEC 3.9).
export const init: ClientInit = startPlugins;

// A view that fails to load says why, rather than SvelteKit's "Internal
// Error". The commands reject with the message itself. SvelteKit's own
// errors, such as a 404, keep their message.
export const handleError: HandleClientError = ({ kind, error }) =>
	kind === 'unknown' ? { message: String(error) } : undefined;
