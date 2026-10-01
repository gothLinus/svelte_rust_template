set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load

just := just_executable()
migrations := "server/migrations"
types := "web/src/lib/types/generated"
image := env("IMAGE_NAME", "svelte-rust-template")
opener := if os() == "macos" { "open" } else { "xdg-open" }
mailpit_url := "http://localhost:" + env("MAILPIT_UI_PORT", "8025")
rustfs_console_url := "http://localhost:" + env("RUSTFS_CONSOLE_PORT", "9001")
coverage_ignore := "(/tests/|src/main\\.rs|src/bin/)"

alias d := dev
alias t := test
alias f := fmt

default:
    @{{ just }} --list --unsorted

# First run: check tools, create .env, start Postgres, RustFS and Mailpit, migrate, install web deps
[group('setup')]
setup: doctor env
    {{ just }} db-up migrate
    cd web && bun install

# Copy .env.example to .env with generated SECRET_KEY and STORAGE_SECRET_KEY, unless .env exists
[group('setup')]
env:
    #!/usr/bin/env bash
    set -euo pipefail
    test -f .env && exit 0
    key="$(openssl rand -base64 32)"
    storage_key="$(openssl rand -hex 20)"
    sed -e "s|^SECRET_KEY=.*|SECRET_KEY=${key}|" \
        -e "s|^STORAGE_SECRET_KEY=.*|STORAGE_SECRET_KEY=${storage_key}|" .env.example > .env
    echo "Created .env from .env.example with a new SECRET_KEY and STORAGE_SECRET_KEY"

# Check that every tool the recipes need is installed and Docker is running
[group('setup')]
doctor:
    #!/usr/bin/env bash
    set -uo pipefail
    missing=0
    need() {
        if command -v "$1" >/dev/null 2>&1; then
            printf '  \e[32m✓\e[0m %s\n' "$1"
        else
            printf '  \e[31m✗\e[0m %-15s install: %s\n' "$1" "$2"
            missing=1
        fi
    }
    need cargo          "https://rustup.rs"
    need bun            "https://bun.sh"
    need docker         "https://docs.docker.com/get-docker/"
    need openssl        "your package manager"
    need sqlx           "just tools"
    need cargo-llvm-cov "just tools"
    need cargo-deny     "just tools"
    need cargo-machete  "just tools"
    if ! command -v gitleaks >/dev/null 2>&1; then
        printf '  \e[33m-\e[0m %-15s optional, for `just secrets`: https://github.com/gitleaks/gitleaks\n' gitleaks
    fi
    if command -v docker >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then
        printf '  \e[31m✗\e[0m Docker is installed but the daemon is not running\n'
        missing=1
    fi
    exit $missing

# Install the cargo tools the recipes use
[group('setup')]
tools:
    cargo install --locked sqlx-cli --no-default-features --features postgres,rustls
    cargo install --locked cargo-llvm-cov cargo-deny cargo-machete
    cd server && rustup component add llvm-tools-preview

# Start the database and RustFS, then the API and the Vite dev server together (Ctrl-C stops both)
[group('dev')]
dev: db-up
    #!/usr/bin/env bash
    set -uo pipefail
    # Either process exiting takes the other down.
    trap 'trap - INT TERM EXIT; kill 0 2>/dev/null' INT TERM EXIT
    (cd server && cargo run -p api; kill $$) &
    (cd web && bun run dev; kill $$) &
    wait

# Run only the API on BIND_ADDRESS
[group('dev')]
[working-directory('server')]
api: db-up
    cargo run -p api

# Run only the Vite dev server (proxies /api to the API)
[group('dev')]
[working-directory('web')]
web:
    bun run dev

# Give an existing account the admin role: just create-admin alice@example.com
[group('dev')]
[working-directory('server')]
create-admin email: db-up
    cargo run --quiet -p api -- create-admin {{ quote(email) }}

# Open Mailpit's inbox, where development mail lands
[group('dev')]
mail:
    {{ opener }} {{ mailpit_url }}

# Open the RustFS console, where uploaded files are stored
[group('dev')]
storage:
    {{ opener }} {{ rustfs_console_url }}

# Start Postgres, RustFS and Mailpit and wait until they are healthy
[group('db')]
db-up: env
    @docker compose --profile mail --progress quiet up -d --wait postgres rustfs mailpit

# Stop the containers, keeping their data
[group('db')]
db-down:
    docker compose --profile mail --profile prod down

# Drop all data, stored files included, and start from a freshly migrated database
[confirm("This deletes every row in the local database and every stored file. Continue? [y/N]")]
[group('db')]
db-reset:
    docker compose --profile mail --profile prod down --volumes
    {{ just }} db-up migrate

# Apply pending migrations (the API also does this on startup)
[group('db')]
migrate: db-up
    sqlx migrate run --source {{ migrations }}

# Revert the most recent migration
[group('db')]
migrate-down: db-up
    sqlx migrate revert --source {{ migrations }}

# Create a reversible migration pair: just migration create_things
[group('db')]
migration name:
    sqlx migrate add -r --source {{ migrations }} {{ name }}

# Open psql in the Postgres container
[group('db')]
psql: db-up
    docker compose exec postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"

# Copy the notes example as a new resource: just new-resource project projects
[group('gen')]
new-resource singular plural:
    bun scripts/new-resource.ts {{ quote(singular) }} {{ quote(plural) }}

# Regenerate the .sqlx query cache so the server builds without a database (commit it)
[group('gen')]
[working-directory('server')]
sqlx-prepare: db-up
    cargo sqlx prepare --workspace -- --all-targets

# Regenerate the frontend's TypeScript from /proto and the domain's field limits
[group('gen')]
gen-types:
    rm -rf {{ types }}/*.ts {{ types }}/api
    cd server && cargo run --quiet -p application --bin export-types -- ../{{ types }}
    cd web && bun run gen:proto

# Fail if the committed TypeScript differs from what /proto and the domain generate
[group('gen')]
check-types:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp -d)"
    trap 'rm -r "$tmp"' EXIT
    mkdir "$tmp/expected"
    (cd server && cargo run --quiet -p application --bin export-types -- "$tmp/expected")
    (cd web && bun run --silent gen:proto --output "$tmp/buf")
    mv "$tmp/buf/src/lib/types/generated/api" "$tmp/expected/api"
    diff -r "$tmp/expected" {{ types }} || { echo "TypeScript types are stale: run 'just gen-types'" >&2; exit 1; }

# Format everything
[group('quality')]
fmt:
    cd server && cargo fmt --all
    cd web && bun run format

# Clippy (pedantic, warnings are errors), Prettier, ESLint and buf lint (for /proto)
[group('quality')]
lint:
    cd server && cargo clippy --workspace --all-targets --all-features -- -D warnings
    cd web && bun run lint

# Type-check both halves without running anything
[group('quality')]
check:
    cd server && cargo check --workspace --all-targets
    cd web && bun run check

# Run every test: the server's against Postgres, then the frontend's
[group('quality')]
test: server-test web-test

# Server tests; each #[sqlx::test] gets its own database: just server-test notes
[group('quality')]
[working-directory('server')]
server-test *args: db-up
    cargo test --workspace {{ args }}

# Frontend unit tests: just web-test auth
[group('quality')]
[working-directory('web')]
web-test *args:
    bun run test {{ args }}

# Measure coverage and fail below the thresholds; HTML reports in server/target/llvm-cov/html and web/coverage
[group('quality')]
coverage: server-coverage web-coverage

[group('quality')]
[working-directory('server')]
server-coverage: db-up
    cargo llvm-cov --workspace --ignore-filename-regex '{{ coverage_ignore }}' --html --fail-under-lines 90
    cargo llvm-cov report --ignore-filename-regex '{{ coverage_ignore }}' --summary-only

[group('quality')]
[working-directory('web')]
web-coverage:
    bun run test:coverage

# Audit dependencies: advisories, licenses, duplicates, unused crates
[group('quality')]
[working-directory('server')]
audit:
    cargo deny check
    cargo machete

# Everything CI runs, in order. Needs Docker for Postgres.
[group('quality')]
ci: db-up
    cd server && cargo fmt --all --check
    {{ just }} lint check check-types
    cd server && cargo sqlx prepare --workspace --check -- --all-targets
    {{ just }} coverage audit
    cd web && bun run build

# What `ci` leaves out for time or tools: secrets, frontend advisories, the MSRV, the image
[group('quality')]
ci-extra: secrets web-audit msrv smoke

# Scan the working tree and the history for committed secrets
[group('quality')]
secrets:
    gitleaks dir . --no-banner --redact --config .gitleaks.toml
    gitleaks git . --no-banner --redact --config .gitleaks.toml

# Known vulnerabilities in the frontend's packages (the server's are in `just audit`)
[group('quality')]
[working-directory('web')]
web-audit:
    bun audit --audit-level=moderate

# Build with the oldest Rust the workspace claims to support (`rust-version`)
[group('quality')]
[working-directory('server')]
msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    msrv="$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml).0"
    rustup toolchain list | grep -q "^$msrv" || rustup toolchain install "$msrv" --profile minimal
    SQLX_OFFLINE=true cargo "+$msrv" check --workspace --all-targets --locked

# Build the production image, boot it against a scratch database and probe it like a browser
[group('quality')]
smoke: docker-build db-up
    #!/usr/bin/env bash
    set -euo pipefail
    db="smoke_$(date +%s)"
    port=18080
    network="${COMPOSE_PROJECT_NAME:-svelte_rust_template}_default"
    base="http://localhost:$port"
    docker compose exec -T postgres createdb -U "$POSTGRES_USER" "$db"
    cid=""
    work="$(mktemp -d)"
    cleanup() {
        [ -n "$cid" ] && docker rm -f "$cid" >/dev/null
        docker compose exec -T postgres dropdb -U "$POSTGRES_USER" --if-exists "$db"
        rm -r "$work"
    }
    trap cleanup EXIT
    cid="$(docker run -d --network "$network" -p "127.0.0.1:$port:3000" \
        -e DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@postgres:5432/$db" \
        -e APP_URL="$base" -e MAIL_TRANSPORT=log -e MAIL_FROM="Smoke <smoke@example.com>" \
        -e STORAGE_ENDPOINT=http://rustfs:9000 -e STORAGE_BUCKET=smoke \
        -e STORAGE_ACCESS_KEY="${STORAGE_ACCESS_KEY:-rustfsadmin}" \
        -e STORAGE_SECRET_KEY="${STORAGE_SECRET_KEY:-rustfsadmin}" \
        -e SECRET_KEY="$(openssl rand -base64 32)" {{ image }}:latest)"
    fail() { echo "smoke: $1" >&2; docker logs "$cid" | tail -20 >&2; exit 1; }
    for _ in $(seq 60); do
        [ "$(curl -s -o /dev/null -w '%{http_code}' "$base/health/ready")" = 200 ] && break
        sleep 0.5
    done
    [ "$(curl -s -o /dev/null -w '%{http_code}' "$base/health/ready")" = 200 ] || fail "never became ready"
    headers="$(curl -s -D - -o /dev/null "$base/")"
    grep -qi '^content-security-policy:' <<<"$headers" || fail "no CSP on the SPA"
    grep -qi '^x-frame-options: deny' <<<"$headers" || fail "no X-Frame-Options"
    grep -qi '^content-type: text/html' <<<"$headers" || fail "the SPA is not served"
    code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$base/api/v1/auth/logout")"
    [ "$code" = 403 ] || fail "a POST without X-Requested-With got $code, not 403"
    # RegisterRequest { email = 1, username = 2, password = 3 }
    printf '\x0a\x11smoke@example.com\x12\x05smoke\x1a\x1ccorrect horse battery staple' > "$work/register"
    proto=(-H 'Content-Type: application/x-protobuf' -H 'X-Requested-With: fetch' -H "Origin: $base")
    code="$(curl -s -o /dev/null -w '%{http_code}' -c "$work/jar" "${proto[@]}" \
        --data-binary @"$work/register" "$base/api/v1/auth/register")"
    [ "$code" = 201 ] || fail "registering answered $code, not 201"
    code="$(curl -s -o /dev/null -w '%{http_code}' -b "$work/jar" "$base/api/v1/me")"
    [ "$code" = 200 ] || fail "the new session answered $code on /me"
    code="$(curl -s -o /dev/null -w '%{http_code}' -b "$work/jar" -H 'Content-Type: text/plain' \
        -H 'X-Requested-With: fetch' -H "Origin: $base" --data-binary 'smoke' \
        "$base/api/v1/files?name=smoke.txt")"
    [ "$code" = 201 ] || fail "uploading a file answered $code, not 201"
    echo "smoke: the image boots, serves the SPA with its headers, refuses CSRF, signs up and stores a file"

# Build the SPA into web/build and a release binary into server/target/release/api
[group('build')]
build:
    cd web && bun run build
    cd server && SQLX_OFFLINE=true cargo build --release --locked -p api --bin api

# Serve the release build with the SPA on http://localhost:3000, like production
[group('build')]
[working-directory('server')]
serve: build db-up
    STATIC_DIR=../web/build APP_URL=http://localhost:3000 ./target/release/api

# Build the production Docker image
[group('build')]
docker-build:
    docker build -t {{ image }}:latest .

# Run the production image with Postgres, RustFS and Mailpit on http://localhost:$APP_PORT: just up -d
[group('build')]
up *args:
    docker compose --profile prod up --build {{ args }}

# Update dependencies within their semver ranges
[group('build')]
update:
    cd server && cargo update
    cd web && bun update

# Remove build output
[group('build')]
clean:
    cd server && cargo clean
    rm -rf web/build web/.svelte-kit web/coverage
