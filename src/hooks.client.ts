import type { HandleClientError } from '@sveltejs/kit';

// A view that fails to load says why, rather than SvelteKit's "Internal
// Error". The commands reject with the message itself.
export const handleError: HandleClientError = ({ error }) => ({ message: String(error) });
