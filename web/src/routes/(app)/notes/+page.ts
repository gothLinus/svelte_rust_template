import { createApi } from '$lib/api';
import type { NoteScope } from '$lib/api';
import { NOTES } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

const PAGE_SIZE = 12;

export const load: PageLoad = async ({ fetch, parent, url, depends }) => {
	await parent();
	depends(NOTES);

	const scope: NoteScope = url.searchParams.get('scope') === 'all' ? 'all' : 'mine';
	const after = url.searchParams.get('after');
	const notes = await fromApi(() =>
		createApi(fetch).notes.list({ scope, after: after ?? undefined, limit: PAGE_SIZE })
	);
	return { notes, scope, after };
};
