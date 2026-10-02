/**
 * `just new-resource <singular> <plural>`: copies the `notes` resource across every layer
 * as a new user-owned resource and registers it wherever notes are registered. The fields
 * and rules stay as notes' `title` and `body`. Run it on a clean working tree.
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const root = join(import.meta.dir, '..');
const [singular, plural] = process.argv.slice(2);

if (!singular || !plural || !/^[a-z]+$/.test(singular) || !/^[a-z]+$/.test(plural)) {
	fail('usage: just new-resource <singular> <plural>, lowercase letters only, e.g. project projects');
}
if (singular === plural || [singular, plural].some((name) => ['note', 'notes'].includes(name))) {
	fail('the names must differ from each other and from note/notes');
}

const capitalize = (word: string) => word[0].toUpperCase() + word.slice(1);
const names = {
	note: singular,
	notes: plural,
	Note: capitalize(singular),
	Notes: capitalize(plural),
	NOTE: singular.toUpperCase(),
	NOTES: plural.toUpperCase()
};

/** Renames `note`/`notes` in every case as words or identifier parts, but not `notebook-pen` or `footnote`. */
function rename(text: string): string {
	return text
		.replace(/(?<![a-z])note(s?)(?![a-z])/g, (_, s: string) => (s ? names.notes : names.note))
		.replace(/Note(s?)(?![a-z])/g, (_, s: string) => (s ? names.Notes : names.Note))
		.replace(/(?<![A-Z])NOTE(S?)(?![A-Z])/g, (_, s: string) => (s ? names.NOTES : names.NOTE));
}

function fail(message: string): never {
	console.error(`new-resource: ${message}`);
	process.exit(1);
}

const path = (relative: string) => join(root, relative);
const read = (relative: string) => readFileSync(path(relative), 'utf8');
const written: string[] = [];

function write(relative: string, content: string) {
	mkdirSync(dirname(path(relative)), { recursive: true });
	writeFileSync(path(relative), content);
	written.push(relative);
}

function copy(relative: string) {
	const target = rename(relative);
	if (existsSync(path(target))) fail(`${target} exists already`);
	if (statSync(path(relative)).isDirectory()) {
		for (const entry of readdirSync(path(relative))) copy(join(relative, entry));
		return;
	}
	write(target, rename(read(relative)));
}

function edit(relative: string, change: (content: string) => string) {
	const before = read(relative);
	const after = change(before);
	if (after === before) fail(`nothing to register in ${relative}: has its notes entry moved?`);
	write(relative, after);
}

/** Repeats each run of lines matching `pattern`, renamed, adding a comma to a list's last item. */
function duplicateLines(pattern: RegExp) {
	return (content: string) => {
		const lines = content.split('\n');
		const out: string[] = [];
		for (let i = 0; i < lines.length; i++) {
			const closesList = /^\s*[\]})]/.test(lines[i + 1] ?? '') && /[}\])'"\w]$/.test(lines[i]);
			out.push(pattern.test(lines[i]) && closesList ? `${lines[i]},` : lines[i]);
			if (pattern.test(lines[i]) && !pattern.test(lines[i + 1] ?? '')) {
				let start = i;
				while (start > 0 && pattern.test(lines[start - 1])) start--;
				out.push(...lines.slice(start, i + 1).map(rename));
			}
		}
		return out.join('\n');
	};
}

/** Repeats the braced block starting at `start`, with its doc comment, renamed. */
function duplicateBlock(start: RegExp) {
	return (content: string) => {
		const lines = content.split('\n');
		const first = lines.findIndex((line) => start.test(line));
		if (first < 0) return content;
		let begin = first;
		while (begin > 0 && /^\s*(\/\/\/|\/\*\*|\*|\/\/)/.test(lines[begin - 1])) begin--;
		let depth = 0;
		let end = first;
		for (; end < lines.length; end++) {
			for (const char of lines[end]) {
				if (char === '{') depth++;
				if (char === '}') depth--;
			}
			if (depth === 0 && end >= first && lines[end].includes('}')) break;
		}
		const block = lines.slice(begin, end + 1).map(rename);
		return [...lines.slice(0, end + 1), '', ...block, ...lines.slice(end + 1)].join('\n');
	};
}

const languages = readdirSync(path('locales')).filter((entry) =>
	statSync(path(join('locales', entry))).isDirectory()
);
const catalog = languages.flatMap((language) =>
	['server', 'web']
		.map((side) => `locales/${language}/${side}/notes.ftl`)
		.filter((file) => existsSync(path(file)))
);
for (const side of ['server', 'web']) {
	if (!catalog.includes(`locales/en/${side}/notes.ftl`)) {
		fail(`locales/en/${side}/notes.ftl is missing: the notes keep their messages there`);
	}
}

for (const source of [
	...catalog,
	'server/crates/domain/src/note',
	'server/crates/application/src/notes',
	'server/crates/infrastructure/src/db/repositories/notes.rs',
	'server/crates/api/src/wire/notes.rs',
	'server/crates/api/src/routes/notes.rs',
	'proto/api/v1/notes.proto',
	'server/crates/application/tests/unit/notes.rs',
	'server/crates/infrastructure/tests/postgres/notes.rs',
	'server/crates/api/tests/http/notes.rs',
	'web/src/lib/api/notes.ts',
	'web/src/lib/components/note-form.svelte',
	'web/src/lib/components/note-dialog.svelte',
	'web/src/routes/(app)/notes',
	'web/tests/components/notes.test.ts'
]) {
	copy(source);
}

const latest = readdirSync(path('server/migrations'))
	.map((file) => BigInt(file.split('_')[0]))
	.reduce((max, version) => (version > max ? version : max), 0n);
const now = BigInt(
	new Date()
		.toISOString()
		.replace(/[-:T]/g, '')
		.slice(0, 14)
);
const stamp = (now > latest ? now : latest + 1n).toString();
const migration = `server/migrations/${stamp}_create_${plural}`;
const copiedTable = rename(read('server/migrations/20260925000006_notes.up.sql')).replace(
	/^-- The example resource\. Copy this migration when adding one\.\n/,
	`-- ${names.Notes}, copied from the notes: rename the columns to its fields.\n`
);
// The notes got their version later (`*_note_versions.up.sql`); a new table has it from the start.
const lastColumn = /(\n {4}updated_at timestamptz not null default now\(\))\n\);/;
if (!lastColumn.test(copiedTable)) {
	fail('the notes migration no longer ends its columns with updated_at: add the version column by hand');
}
const table = `${copiedTable.replace(
	lastColumn,
	'$1,\n    version bigint not null default 1 check (version >= 1)\n);'
)}
create trigger ${plural}_bump_version before update on ${plural}
    for each row execute function bump_version();
`;
write(
	`${migration}.up.sql`,
	`${table}
-- Keep in step with \`for_each_permission!\` and DEFAULT_USER_PERMISSIONS.
insert into permissions (name, description) values
    ('${plural}:read', 'Read your own ${plural}'),
    ('${plural}:write', 'Create, edit and delete your own ${plural}'),
    ('${plural}:manage', 'Read, edit and delete anyone''s ${plural}');

insert into role_permissions (role, permission) values
    ('admin', '${plural}:read'),
    ('admin', '${plural}:write'),
    ('admin', '${plural}:manage'),
    ('user', '${plural}:read'),
    ('user', '${plural}:write');
`
);
write(
	`${migration}.down.sql`,
	`delete from permissions where name in ('${plural}:read', '${plural}:write', '${plural}:manage');

drop table ${plural};
`
);

// Permissions
edit('server/crates/domain/src/rbac/permission.rs', duplicateLines(/^\s+Notes(Read|Write|Manage) =>/));
edit('server/crates/domain/src/rbac/role.rs', (content) =>
	content.replace(
		/(DEFAULT_USER_PERMISSIONS[^;]*Permission::NotesWrite)/,
		`$1, Permission::${names.Notes}Read, Permission::${names.Notes}Write`
	)
);
edit('proto/api/v1/common.proto', (content) => {
	// Enum values are appended, never renumbered.
	const numbers = [...content.matchAll(/^\s+PERMISSION_\w+ = (\d+);/gm)].map((m) => Number(m[1]));
	let next = Math.max(...numbers);
	const lines = content.split('\n');
	const last = lines.findLastIndex((line) => /^\s+PERMISSION_\w+ = \d+;/.test(line));
	const added = lines
		.filter((line, i) => /PERMISSION_NOTES_|`notes:/.test(line) && i <= last)
		.map((line) => rename(line).replace(/= \d+;/, () => `= ${++next};`));
	lines.splice(last + 1, 0, ...added);
	return lines.join('\n');
});
edit('server/crates/api/src/wire/common.rs', duplicateLines(/PermissionName::Notes\w+ =>/));

// Modules, unit of work, services, routes
edit('server/crates/domain/src/lib.rs', duplicateLines(/^pub mod note;$/));
edit('server/crates/application/src/lib.rs', duplicateLines(/^pub mod notes;$/));
edit('server/crates/application/src/context.rs', (content) =>
	duplicateLines(/Repository<Note>/)(duplicateLines(/^\s+note::Note,$/)(content))
);
edit('server/crates/application/src/services.rs', duplicateLines(/notes::NoteService|NoteService/));
edit('server/crates/infrastructure/src/db/repositories/mod.rs', duplicateLines(/^mod notes;$/));
edit('server/crates/api/src/wire/mod.rs', duplicateLines(/^mod notes;$/));
edit('server/crates/api/src/routes/mod.rs', duplicateLines(/^mod notes;$|\.merge\(notes::routes\(\)\)/));

// Field limits and the data export
edit('server/crates/application/src/types.rs', (content) =>
	duplicateLines(/"MAX_NOTE_/)(content).replace(/(use domain::\{[^}]*\bnote),/, `$1, ${singular},`)
);
edit('server/crates/application/src/account/service.rs', (content) =>
	content.replace(
		/^([ \t]*)(export\s*\.add_owned::<domain::note::Note>\([^)]*\)\s*\.await\?;)$/m,
		(statement, indent: string, call: string) => `${statement}\n${indent}${rename(call)}`
	)
);
edit('server/crates/application/src/export.rs', duplicateLines(/^\s+"notes",$/));

// Tests
for (const main of [
	'server/crates/application/tests/unit/main.rs',
	'server/crates/infrastructure/tests/postgres/main.rs',
	'server/crates/api/tests/http/main.rs'
]) {
	edit(main, duplicateLines(/^mod notes;$/));
}
edit('server/crates/application/tests/unit/support/memory.rs', (content) =>
	duplicateBlock(/^impl Repository<Note> for Mem \{/)(
		duplicateLines(
			/^\s+note::\{NewNote|^\s+pub notes: BTreeMap|^\s+notes: BTreeMap::new\(\),|state\.notes\.retain\(/
		)(content)
	)
);

// Frontend
edit('web/src/lib/types/api.ts', duplicateLines(/notes_pb/));
edit('web/src/lib/api/index.ts', duplicateLines(/from '\.\/notes'|notes: notesApi/));
edit('web/src/lib/auth/permissions.ts', (content) =>
	duplicateBlock(/^export const notePolicy/)(
		content.replace(/import \{ type Note,/, `import { type Note, type ${names.Note},`)
	)
);
edit('web/src/lib/auth/index.ts', (content) =>
	content.replace(/\bnotePolicy,/, `notePolicy, ${singular}Policy,`)
);
edit('web/src/lib/helpers/dependencies.ts', duplicateLines(/^export const NOTES = /));
edit('web/src/lib/helpers/navigation.ts', duplicateLines(/^\s+page\(.*'\/notes'/));
edit('web/src/lib/forms/validation.ts', (content) =>
	duplicateBlock(/^export const noteBody/)(
		duplicateBlock(/^export const noteTitle/)(duplicateLines(/^\s+MAX_NOTE_\w+,$/)(content))
	)
);
edit('web/tests/helpers.ts', (content) =>
	duplicateBlock(/^export function note\(/)(
		duplicateLines(/^\s+type Note,$|^\s+NoteSchema,$/)(content).replace(
			/(Permission\.NOTES_WRITE)\]/,
			`$1, Permission.${names.NOTES}_READ, Permission.${names.NOTES}_WRITE]`
		)
	)
);

console.log(`new-resource: ${plural} copied from notes (${written.length} files written):`);
for (const file of written) console.log(`  ${file}`);
console.log(`
Next:
  just migrate && just sqlx-prepare && just gen-types && just fmt
  just ci
Then rename the copied fields (title, body) and rules to ${singular}'s own.${
	languages.length > 1
		? `\nThe messages were copied into every language of locales/ with the ids renamed; translate\ntheir words for ${singular} (\`cargo test -p i18n\` and \`bun run test catalog\` check the ids).`
		: ''
}`);
