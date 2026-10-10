import { dev } from '$app/env';
import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { track, type Command } from '#lib/slow-calls.js';

/** A command the window called while it started, timed from when it began loading. */
export interface StartupCall {
	cmd: string;
	start: number;
	ms: number;
}

/**
 * In a dev build, the commands the window calls until its first view is up,
 * for the startup details (`#lib/startup.js`), which close the list then.
 */
export const startupCalls = { open: dev, calls: [] as StartupCall[] };

/** Every call to the backend: timed for the slow-call toast, and at startup. */
export function invoke<T>(cmd: Command, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
	const call = track(cmd, tauriInvoke<T>(cmd, args, options));
	if (!startupCalls.open) return call;
	const start = performance.now();
	return call.finally(() => startupCalls.calls.push({ cmd, start, ms: performance.now() - start }));
}

/**
 * The subscription to a backend event, which hands the handler the payload
 * as sent. It resolves to the function that stops listening.
 */
export const event =
	<T = void>(name: string) =>
	(handler: (payload: T) => void): Promise<UnlistenFn> =>
		listen<T>(name, (e) => handler(e.payload));

/**
 * The teardown of subscriptions that may still be starting: each stops once
 * it is in place, and one that failed to start is let be.
 */
export const stopAll =
	(...offs: Promise<UnlistenFn>[]) =>
	(): void => {
		for (const off of offs)
			off.then(
				(stop) => stop(),
				() => {}
			);
	};
