import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { searchForWorkspaceRoot } from 'vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ fallback: 'index.html' }),
			// Emitted as a <meta> tag with hashes of SvelteKit's inline bootstrap script. The Rust
			// server adds the directives a <meta> policy cannot carry, e.g. `frame-ancestors`.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					// bits-ui positions popovers with inline `style` attributes.
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:'],
					'font-src': ['self'],
					'connect-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['self']
				}
			}
		})
	],
	server: {
		// Vite's default (this package), plus the translation catalog `/locales`, which is
		// shared with the server and lies outside it. Nothing else of the repository.
		fs: { allow: [searchForWorkspaceRoot(process.cwd()), '../locales'] },
		proxy: {
			'/api': 'http://127.0.0.1:3000'
		}
	},
	test: {
		projects: [
			{
				extends: true,
				test: {
					name: 'unit',
					include: ['tests/lib/**/*.test.ts', 'tests/routes/**/*.test.ts'],
					environment: 'node',
					setupFiles: ['tests/i18n-setup.ts']
				}
			},
			{
				extends: true,
				plugins: [svelteTesting()],
				test: {
					name: 'components',
					include: ['tests/components/**/*.test.ts'],
					environment: 'jsdom',
					setupFiles: ['tests/i18n-setup.ts', 'tests/components/setup.ts']
				}
			}
		],
		coverage: {
			provider: 'v8',
			include: ['src/lib/**/*.{ts,svelte}', 'src/routes/**/*.{ts,svelte}'],
			exclude: [
				'src/lib/components/ui/**',
				'src/lib/hooks/**',
				'src/lib/types/generated/**',
				'src/lib/utils.ts'
			],
			reporter: ['text', 'html'],
			// `bun run test:coverage` (and `just coverage`) fail below these.
			// Components and pages included; v8 counts every `{#if}` and `?:` in the compiled
			// Svelte output as a branch, hence the lower branch bar overall.
			thresholds: {
				lines: 90,
				functions: 90,
				statements: 90,
				branches: 75,
				'src/lib/**/*.ts': { lines: 90, functions: 90, statements: 90, branches: 85 }
			}
		}
	}
});
