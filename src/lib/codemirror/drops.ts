import { getCurrentWebview } from '@tauri-apps/api/webview';

/** What each editor's box, `.md-editor`, does with files dropped on it. */
const dropTargets = new WeakMap<Element, (paths: string[], at: { x: number; y: number }) => void>();
let listening = false;

/**
 * Files dragged in from the file manager go to Tauri rather than the page,
 * with their paths, so one listener per window finds the editor under the
 * pointer, outlines it while they hover, and hands it the drop.
 */
function listenForDrops() {
	if (listening) return;
	listening = true;
	void getCurrentWebview().onDragDropEvent(({ payload }) => {
		for (const box of document.querySelectorAll('.md-drop')) box.classList.remove('md-drop');
		if (payload.type === 'leave') return;
		const at = payload.position.toLogical(window.devicePixelRatio);
		const box = document.elementFromPoint(at.x, at.y)?.closest('.md-editor');
		const drop = box && dropTargets.get(box);
		if (!drop) return;
		if (payload.type === 'drop') drop(payload.paths, at);
		else box.classList.add('md-drop');
	});
}

/** Hand the files dropped on an editor's box to `drop`; returns the way to stop. */
export function acceptDrops(
	box: Element,
	drop: (paths: string[], at: { x: number; y: number }) => void
) {
	dropTargets.set(box, drop);
	listenForDrops();
	return () => {
		dropTargets.delete(box);
	};
}
