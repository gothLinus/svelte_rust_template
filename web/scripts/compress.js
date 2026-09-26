// Writes a `.br` and a `.gz` next to every compressible file in `build/`, at the highest
// levels: the Rust server sends those to browsers that accept them (see `server/crates/api/src/spa.rs`),
// so it never compresses a static file per request. Runs after `vite build`.

import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { brotliCompress, constants, gzip } from 'node:zlib';
import { promisify } from 'node:util';

const BUILD_DIR = new URL('../build/', import.meta.url).pathname;
const COMPRESSIBLE = /\.(html|js|mjs|css|json|svg|txt|xml|webmanifest|wasm)$/;
const MIN_BYTES = 1024;

const br = promisify(brotliCompress);
const gz = promisify(gzip);

async function compress(source, path) {
	const [brotli, gzipped] = await Promise.all([
		br(source, {
			params: {
				[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
				[constants.BROTLI_PARAM_SIZE_HINT]: source.length
			}
		}),
		gz(source, { level: constants.Z_BEST_COMPRESSION })
	]);
	// A variant that is not smaller is left out; the original is served instead.
	await Promise.all([
		brotli.length < source.length ? writeFile(`${path}.br`, brotli) : undefined,
		gzipped.length < source.length ? writeFile(`${path}.gz`, gzipped) : undefined
	]);
}

const entries = await readdir(BUILD_DIR, { recursive: true, withFileTypes: true });
const files = entries
	.filter((entry) => entry.isFile() && COMPRESSIBLE.test(entry.name))
	.map((entry) => join(entry.parentPath, entry.name));

let compressed = 0;
await Promise.all(
	files.map(async (path) => {
		const source = await readFile(path);
		if (source.length < MIN_BYTES) return;
		await compress(source, path);
		compressed += 1;
	})
);
console.log(`Precompressed ${compressed} of ${files.length} files in build/`);
