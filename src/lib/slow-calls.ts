import { toast } from 'svelte-sonner';
import { m } from '#lib/paraglide/messages.js';

/**
 * Says so when a command keeps the window waiting. Every call to the backend
 * goes through `track`, which times it from its start; one that is still
 * running after SLOW_AFTER shows a toast naming what it is doing, until the
 * last slow one is done. A quick call costs one timer, and nothing runs
 * while no call is slow.
 */

/** How long a call may run before the window says it is still at it. */
const SLOW_AFTER = 3000;

type Label = () => string;

/**
 * What the toast calls a command's work. Null for a call that never toasts:
 * one that shows its own progress, or that runs out of sight, such as the
 * recall while a note is written. A command not listed is still working.
 */
const LABELS: Record<string, Label | null> = {
	launch_steps: null,
	hide_capture: null,
	set_tray_labels: null,
	restart_app: null,
	old_chat_model: null,
	download_embedding_model: null,
	search_meaning: null,
	recall: null,
	map_links: null,
	plugin_data: null,
	save_plugin_data: null,
	browse_plugins: null,
	plugin_details: null,

	save_note: m.slow_saving,
	update_note: m.slow_saving,
	clear_day_ahead: m.slow_saving,
	create_page: m.slow_saving,
	update_page: m.slow_saving,
	finish_page: m.slow_saving,
	rename_page: m.slow_saving,
	note_to_page: m.slow_saving,
	capture_to_page: m.slow_saving,
	set_settings: m.slow_saving,

	get_day: m.slow_loading,
	list_days: m.slow_loading,
	notes_about: m.slow_loading,
	get_note: m.slow_loading,
	list_pages: m.slow_loading,
	get_page: m.slow_loading,
	list_threads: m.slow_loading,
	get_thread: m.slow_loading,
	thread_cards: m.slow_loading,
	threads_for_note: m.slow_loading,
	list_spaces: m.slow_loading,
	get_settings: m.slow_loading,
	embedding_model_info: m.slow_loading,
	plugins_view: m.slow_loading,
	plugin_code: m.slow_loading,

	search: m.slow_searching,
	similar_notes: m.slow_searching,
	map_search: m.slow_searching,
	notes_containing: m.slow_searching,

	note_map: m.slow_map,
	map_categories: m.slow_map,

	rebuild_index: m.slow_rebuilding,

	rename_thread: m.slow_threads,
	keep_out_of_threads: m.slow_threads,
	keep_thread: m.slow_threads,
	dismiss_thread: m.slow_threads,
	put_in_thread: m.slow_threads,
	merge_threads: m.slow_threads,

	move_note: m.slow_moving,
	move_page: m.slow_moving,
	move_attachments: m.slow_moving,

	delete_note: m.slow_deleting,
	delete_page: m.slow_deleting,
	delete_space: m.slow_deleting,
	remove_old_chat_model: m.slow_deleting,

	add_attachments: m.slow_attaching,
	save_attachment: m.slow_attaching,

	set_plugins: m.slow_plugins,
	install_plugin: m.slow_plugins,
	uninstall_plugin: m.slow_plugins,

	create_space: m.slow_spaces,
	rename_space: m.slow_spaces,
	set_active_space: m.slow_spaces
};

interface Call {
	cmd: string;
	start: number;
	slow: boolean;
}

/** The calls under way, oldest first, as a Map keeps them. */
const running = new Map<number, Call>();
let next = 0;

const TOAST = 'slow-calls';
/** Counts the seconds on the toast while it shows. */
let ticker: ReturnType<typeof setInterval> | undefined;

/** The call, unchanged, timed until it settles. */
export function track<T>(cmd: string, call: Promise<T>): Promise<T> {
	if (LABELS[cmd] === null) return call;
	const id = next++;
	const entry: Call = { cmd, start: performance.now(), slow: false };
	running.set(id, entry);
	const timer = setTimeout(() => {
		entry.slow = true;
		show();
	}, SLOW_AFTER);
	return call.finally(() => {
		clearTimeout(timer);
		running.delete(id);
		if (entry.slow) show();
	});
}

/** The toast for the oldest slow call, with how many more are slow; none once they are all done. */
function show() {
	const slow = [...running.values()].filter((call) => call.slow);
	if (slow.length === 0) {
		clearInterval(ticker);
		ticker = undefined;
		toast.dismiss(TOAST);
		return;
	}
	ticker ??= setInterval(show, 1000);
	const [oldest] = slow;
	const seconds = Math.round((performance.now() - oldest.start) / 1000);
	const others = slow.length - 1;
	const taking = m.slow_taking({ seconds });
	toast.loading((LABELS[oldest.cmd] ?? m.slow_working)(), {
		id: TOAST,
		description: others > 0 ? `${taking} ${m.slow_more({ count: others })}` : taking,
		duration: Number.POSITIVE_INFINITY
	});
}
