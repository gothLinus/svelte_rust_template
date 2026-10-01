# svelte_rust_template

A full-stack starter with accounts, sign-in and roles built in.

- **Server:** Rust 2024, Axum, SQLx and PostgreSQL, in layered crates.
- **Web:** a SvelteKit 5 single-page app (Tailwind v4, shadcn-svelte) served by the API.

```
server/    Cargo workspace: domain, application, infrastructure, i18n, api, proto
web/       SvelteKit SPA, adapter-static
proto/     Protocol Buffers messages, the wire contract
locales/   Fluent translations for the server and the web app
```

## Features

- Registration and sign-in with email, username or phone number plus a password.
- Passwordless sign-in: passkeys, magic links, email codes, SMS and WhatsApp codes (Twilio).
- Social sign-in through OAuth 2.0 / OpenID Connect: Google, Apple, GitHub, Microsoft.
- Two-step verification: authenticator apps, security keys, recovery codes.
- Email verification, password reset, session list with revocation.
- Account deletion and a JSON export of the user's data.
- Roles and permissions with ownership rules. Admins change roles, disable accounts, see
  a user's sessions and sign them out of one device or everywhere.
- An audit log of security events (sign-ins, failed attempts, changed sign-in methods, role
  changes): users see their own on the security page, holders of `audit:read` see everyone's.
- Files: upload, download, rename and delete, streamed to an S3-compatible object store
  (RustFS in development) with a size limit and a per-account quota.
- Problem Details errors (RFC 9457) with field-level validation, keyset pagination.
- Optimistic concurrency: every note carries a version, sent as `ETag`; updates and deletes
  with `If-Match` are refused with `412` if someone changed it since.
- Translations in Fluent files, English and German included, with a language switcher.
  Each account keeps its language, and mail and texts to it follow that language.
- `just new-resource` scaffolds a new resource in every layer, copied from the `notes` example.

## Quick start

Requires Rust, [bun](https://bun.sh), Docker and [just](https://github.com/casey/just).
`just doctor` shows what is missing and `just tools` installs the cargo helpers.

```sh
just setup    # create .env, start Postgres, RustFS and Mailpit, migrate, install web dependencies
just dev      # API on :3000, web app on http://localhost:5173
```

Mail lands in Mailpit at http://localhost:8025 (`just mail`). Uploaded files land in RustFS,
whose console is at http://localhost:9001 (`just storage`; sign in with `STORAGE_ACCESS_KEY`
and `STORAGE_SECRET_KEY` from `.env`). To make an account an admin:

```sh
just create-admin you@example.com
```

Configuration is read from `.env`; `.env.example` lists every variable. The server
validates them at startup and reports all problems at once. Development shortcuts such as
plain HTTP and mail written to the log are accepted only while `APP_URL` is localhost.

## Architecture

```
api ──────────► application ──────────► domain ◄────────── infrastructure
(axum, wire)    (services, policies)    (entities, ports)  (sqlx, argon2, smtp)
                                           ▲
                                         i18n (Fluent catalog)
```

Dependencies point inward and are enforced by `Cargo.toml`:

- `domain` holds entities, value objects and ports (traits). It has no I/O and no framework.
- `application` holds the services and policies. It depends only on `domain`.
- `infrastructure` implements the ports: Postgres repositories, S3 object store, hashing,
  mail, config.
- `i18n` implements the translator port from the catalog in `locales/`.
- `api` is the composition root: routes, middleware, wire conversion, startup.

Services are generic over a single `Adapters` type family, so dispatch is static, and
database work goes through a unit-of-work port. Start reading at `domain::repository`,
`application::crud` and `api::wire`; `cargo doc --no-deps --open` in `server/` builds the
reference.

The web app is a static SPA: `load` functions and guards run in the browser and are
conveniences only, because the API enforces access. In production the API serves the build
from the same origin; in development Vite proxies `/api`.

## Commands

| Command | Purpose |
| --- | --- |
| `just setup` / `just dev` | First run / API and Vite dev server together |
| `just db-up` / `db-down` / `db-reset` | Start, stop or reset the local services; reset also deletes stored files |
| `just migrate` / `migration <name>` | Apply migrations / create a reversible pair |
| `just sqlx-prepare` | Refresh `server/.sqlx` after changing a query |
| `just gen-types` / `check-types` | Regenerate / verify the TypeScript from `proto/` |
| `just new-resource <thing> <things>` | Scaffold a resource from `notes` |
| `just fmt` / `lint` / `check` | Format, lint, type-check |
| `just test` / `coverage` | Run the tests / enforce coverage thresholds |
| `just audit` | Dependency advisories, licenses, unused crates |
| `just ci` | Everything CI runs; `ci-extra` adds secrets, MSRV and image smoke tests |
| `just build` / `serve` | Release build / serve it on :3000 |
| `just docker-build` / `up` | Build and run the production image |
| `just storage` | Open the RustFS console |

## Security

- Sessions are opaque 32-byte tokens in `HttpOnly`, `Secure`, `SameSite=Lax` cookies. Only
  their SHA-256 is stored. They expire when idle or after a fixed lifetime, rotate on
  sign-in and after role changes, and can be revoked individually or all at once.
- Passwords use Argon2id with bounded concurrency; excess sign-ins get a 503.
- Adding or removing a sign-in method, changing the email address, deleting the account
  and changing roles require a recent sign-in. The first proof of an email address
  removes anything attached to the account before it, so pre-registering someone else's
  address gains nothing.
- Emailed and texted codes are single-use, short-lived, attempt-limited and stored as
  digests. Passkeys and OAuth (PKCE, `state`, ID token checks) follow their specs. A social
  account never takes over an existing account with the same address.
- CSRF: state-changing requests need an `X-Requested-With` header and a matching `Origin`,
  on top of `SameSite`.
- Uploads are checked before any byte is read: permission, name, size, type and the
  account's quota. Downloads are always `Content-Disposition: attachment` under a
  `default-src 'none'` CSP, so an uploaded page or SVG is saved, never run. The bucket is
  private. File contents are deleted through a database queue, so removing a file or an
  account cannot leave them behind.
- Sign-in and password reset answer identically for known and unknown addresses, and so
  does registration with `REQUIRE_EMAIL_VERIFICATION=true`.
- Rate limits per IP and per account on the sensitive endpoints, configurable per
  `RATE_LIMIT_*` variable. Use `RATE_LIMIT_STORE=postgres` with several instances.
- A strict CSP, HSTS, `nosniff`, `X-Frame-Options` and related headers; `no-store` on API
  responses.
- Mail and texts are queued in an outbox table, encrypted with `SECRET_KEY`.
- Security events are written in the same transaction as the change they record, with the
  request's IP and user agent, and kept for `AUDIT_LOG_RETENTION` (90 days by default).

## API

- Everything is under `/api/v1`; `/health/live` and `/health/ready` sit outside it.
- Bodies are Protocol Buffers (`application/x-protobuf`) defined in `proto/api/v1`. The
  server compiles them at build time; `just gen-types` generates the TypeScript.
- Errors are always `application/problem+json`. Switch on the stable `code`; `title`,
  `detail` and field messages are translated per `Accept-Language`.

  ```json
  {
    "type": "about:blank",
    "title": "Unprocessable Entity",
    "status": 422,
    "code": "validation_failed",
    "errors": [{ "field": "email", "code": "invalid_email", "message": "enter a valid email address" }]
  }
  ```

- File contents are raw bytes, streamed both ways: `POST /files?name=<name>` with the file as
  the body (a `Content-Length` is required) and `GET /files/{id}/content`.
- Lists use `?limit=20&after=<cursor>` and return `items` and `next_cursor`, newest first,
  at most 100 per page.
- Versioned resources (notes, and anything copied from them) return their `version` in the
  body and as a strong `ETag` (`"3"`). Send it back as `If-Match` on `PATCH` and `DELETE`; a
  newer version answers `412` with code `stale`. Without `If-Match` the change applies to
  whatever is there.

## Translations

See [locales/README.md](locales/README.md). The server words everything it returns from
`locales/<language>/server`, and the web app words its UI from `locales/<language>/web`.

## Testing

- `domain` and `application` have unit tests against in-memory fakes; time and token
  generation are injected.
- `infrastructure` and `api` have integration tests. Each `#[sqlx::test]` gets its own
  migrated database, and the `api` tests drive the real router.
- The web app uses Vitest for the client, guards, forms and route loads. There are no
  browser end-to-end tests.
- GitHub Actions (`.github/workflows/ci.yml`) runs `just ci` and `just ci-extra` on every
  push to `main` and every pull request.

## Observability

Logs go to stderr as text or JSON (`LOG_FORMAT`, filtered by `RUST_LOG`), each request in a
span with its `x-request-id`. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to export the same spans as
traces and request metrics (`http.server.request.duration`, `http.server.active_requests`,
by route template) to an OpenTelemetry collector over OTLP/HTTP. A caller's W3C
`traceparent` is continued, and problem documents then carry a `traceId`. Without the
endpoint nothing leaves the process.

## Deployment

The `Dockerfile` builds the SPA, compiles the API with `SQLX_OFFLINE=true` and ships both
in a distroless non-root image. `compose.yaml` runs Postgres, RustFS and Mailpit locally, and
its `prod` profile adds the image (`just up`).

In production, set `APP_URL` (https), `DATABASE_URL`, `SECRET_KEY`, `MAIL_TRANSPORT=smtp`,
`SMTP_URL` and `MAIL_FROM`, plus `STORAGE_ENDPOINT`, `STORAGE_BUCKET`, `STORAGE_ACCESS_KEY`
and `STORAGE_SECRET_KEY` for an S3-compatible store (`STORAGE_REGION` for AWS). The server
refuses to start with development settings. Add `TEXT_TRANSPORT=twilio` with the `TWILIO_*`
variables and `TEXT_ALLOWED_COUNTRIES` for SMS, `OAUTH_*` variables per social provider, and
`TRUST_PROXY=true` behind a proxy. Maintenance jobs run on one replica at a time.

## Adding a resource

```sh
just new-resource project projects
just migrate && just sqlx-prepare && just gen-types && just fmt
just ci
```

This copies the `notes` slice into every layer (domain, application, repository, routes,
proto, tests, API module, form, page, translations), adds a migration with the
permissions, and registers it everywhere notes are registered. It takes single lowercase
words and the plural spelled out.

The copy keeps notes' `title` and `body`. Rename them in the migration, the domain types,
the repository queries, `proto/api/v1/<things>.proto`, the wire conversion, the DTOs, the
limits in `application/src/types.rs` and `web/src/lib/forms/validation.ts`, and the
translations. Ownership rules are in the resource's `policy.rs` and `permissions.ts`.
