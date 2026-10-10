/**
 * What a plugin's settings rows are drawn from, apart from the builders in
 * `setting.svelte.ts`, so the row component and the builders that mount it
 * do not import each other.
 */

/** A control on the right of a row, as its builder last set it. */
export type Control =
	| { kind: 'toggle'; value: boolean; disabled: boolean; change?: (value: boolean) => unknown }
	| {
			kind: 'text';
			value: string;
			placeholder: string;
			disabled: boolean;
			change?: (value: string) => unknown;
	  }
	| {
			kind: 'dropdown';
			value: string;
			options: { value: string; label: string }[];
			disabled: boolean;
			change?: (value: string) => unknown;
	  }
	| {
			kind: 'button';
			text: string;
			variant: 'secondary' | 'default' | 'destructive';
			disabled: boolean;
			click?: () => unknown;
	  };

/** What a row draws: a `Setting` changes it, and its row redraws. */
export interface SettingModel {
	name: string;
	desc: string;
	heading: boolean;
	controls: Control[];
}

/** What each container holds, to unmount before it is drawn again. */
const drawn = new WeakMap<HTMLElement, (() => void)[]>();

/** Note what takes away something drawn in `el`, for `clearSettings`. */
export function track(el: HTMLElement, remove: () => void) {
	const list = drawn.get(el) ?? [];
	list.push(remove);
	drawn.set(el, list);
}

/** Take away the settings drawn in `el`, and empty it. */
export function clearSettings(el: HTMLElement) {
	for (const remove of drawn.get(el) ?? []) remove();
	drawn.delete(el);
	el.replaceChildren();
}

/** Run a plugin's handler, which may be async, and say when it fails. */
export function call<T>(handler: ((value: T) => unknown) | undefined, value: T) {
	try {
		void Promise.resolve(handler?.(value)).catch((e) => console.error(e));
	} catch (e) {
		console.error(e);
	}
}
