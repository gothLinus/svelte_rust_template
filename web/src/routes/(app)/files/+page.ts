import { createApi } from '$lib/api';
import type { FileScope } from '$lib/api';
import { FILES } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

const PAGE_SIZE = 20;

export const load: PageLoad = async ({ fetch, parent, url, depends }) => {
	await parent();
	depends(FILES);

	const scope: FileScope = url.searchParams.get('scope') === 'all' ? 'all' : 'mine';
	const after = url.searchParams.get('after');
	const api = createApi(fetch);
	const [files, usage] = await fromApi(() =>
		Promise.all([
			api.files.list({ scope, after: after ?? undefined, limit: PAGE_SIZE }),
			api.files.usage()
		])
	);
	return { files, usage, scope, after };
};
