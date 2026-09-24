import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		// Tauri's devUrl is pinned to 5173, so fail loudly instead of silently
		// moving to another port and leaving the app window blank.
		strictPort: true,
		watch: {
			// Never watch the Rust side: cargo holds build artifacts open while it
			// compiles, which makes the watcher crash with EBUSY on Windows.
			ignored: ['**/src-tauri/**']
		}
	}
});
