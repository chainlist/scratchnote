import { redirect } from '@sveltejs/kit';
import { resolve } from '$app/paths';
import type { PageLoad } from './$types';

// The window opens on today.
export const load: PageLoad = async ({ parent }) => {
	const { today } = await parent();
	redirect(307, resolve(`day/${today}/`));
};
