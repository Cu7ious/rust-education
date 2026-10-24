# Edurust

Interactive computer science lessons. The first course fixture is a short Rust fundamentals slice. The web app is a client of the API; grading and accounts live in the Rust service.

## Layout

- `web/` — Next.js, TypeScript, shadcn/ui, and next-intl. The browser calls `/api/v1`, and Next rewrites that prefix to the API so the session cookie stays on the web origin.
- `api/` — Axum, SQLx, and PostgreSQL. Content is imported from a course package. Postgres stores the immutable release and the learner's attempts.
- `api/fixtures/rust-mini/` — a tiny bilingual course used for local development and tests.

## Local setup

PostgreSQL 16:

```sh
docker compose up -d
```

Copy the environment file and leave the defaults for local development:

```sh
cp .env.example .env
```

`APP_ENV=development` marks a new email as verified so you can log in without a mail server. `DEV_LOG_VERIFICATION_CODES` must stay `false` in production. `SMTP_URL`, `SMS_PROVIDER`, and the Google and GitHub client variables are reserved for a later milestone.

Apply migrations, import the fixture, and start the API on `127.0.0.1:3001`:

```sh
cd api
cargo run --bin import-content -- --path fixtures/rust-mini
cargo run
```

Start the web app on `http://localhost:3000`:

```sh
cd web
npm install
npm run dev
```

Open `http://localhost:3000`. The locale prefix follows the browser language, then falls back to English. Register, log in, open the course, answer the lesson, reload to see the saved result, then log out.

Importing the same package again does nothing. Changing a file without increasing `revision` is rejected.

## Checks

API tests truncate `edurust_test`. They do not wipe the development database.

```sh
cd api
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo sqlx prepare --check
cargo test
```

`cargo test` and `cargo clippy` can use the committed `.sqlx` data with `SQLX_OFFLINE=true`. `cargo sqlx prepare --check` needs a migrated `DATABASE_URL`.

```sh
cd web
npm run format:check
npm run lint
npm run typecheck
```

Regenerate the web types after an API change:

```sh
cd api && cargo run --bin export-openapi
mv openapi.json ../web/openapi.json
cd ../web && npx openapi-typescript openapi.json -o src/lib/api/schema.ts
```
