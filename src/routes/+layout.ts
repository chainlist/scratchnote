export const ssr = false;
export const prerender = true;

// Tauri serves the built files straight off disk, so every route needs to be a
// directory with its own index.html.
export const trailingSlash = 'always';
