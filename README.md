# Decision Copilot

Decision Copilot is a Tauri desktop app for personal decision support. It combines:
- long-running chat memory about the user (local markdown profile files),
- structured decision analysis (options, variables, recommendation),
- a multi-agent committee debate that stress-tests a decision from different viewpoints.

## What Is Implemented

- Chat workspace for general conversation and context gathering.
- Decision workspace with:
  - decision-specific chat,
  - live structured summary panel (`options`, `variables`, `pros_cons`, `recommendation`),
  - status progression through the decision lifecycle.
- Committee workflow:
  - 5 debating agents (`rationalist`, `advocate`, `contrarian`, `visionary`, `pragmatist`),
  - 1 moderator synthesis,
  - quick and full debate modes,
  - streamed tokens and persisted debate transcript.
- Outcome logging and reflection:
  - user logs what happened after choosing,
  - app sends a reflection prompt back through the assistant flow so profile memory can improve.
- Editable local files:
  - profile memory markdown files,
  - committee agent prompt markdown files,
  - optional per-agent model overrides.

## Tech Stack

- Frontend: React 19, Vite, TypeScript, Tailwind CSS v4, Radix UI
- Desktop + backend: Tauri 2, Rust
- Storage:
  - SQLite (`database.sqlite`) for conversations, messages, decisions, debate rounds
  - local markdown files for profile and agent prompts
  - local JSON config for API key/model settings
- LLM routing: OpenRouter Chat Completions API (streaming + tool calls)

## Prerequisites

- Node.js 20+ and npm
- Rust stable and Cargo
- Tauri system prerequisites for your OS
  - Windows: WebView2 runtime

## Quick Start

1. Install dependencies:

```bash
npm install
```

2. Start the desktop app:

```bash
npm run tauri dev
```

This launches Vite at `http://localhost:1420` and opens the Tauri window.
Port `1420` is fixed (`strictPort: true`).

3. First run setup in Settings:
- add your OpenRouter API key (`openrouter.ai/keys`),
- choose a default model (for example `anthropic/claude-sonnet-4-5`),
- save.

## Decision Lifecycle

Decisions move through these statuses:

- `exploring`: decision context is still being collected
- `analyzing`: structured variables/options are taking shape
- `debating`: committee debate is running
- `recommended`: recommendation is ready
- `decided`: user has made a choice
- `reviewed`: user logged the real-world outcome

## Local Data

The app uses Tauri `app_data_dir`.
On Windows this is typically:

`%APPDATA%\com.decisioncopilot.app\`

Key files/folders:
- `database.sqlite`
- `config.json`
- `profile/*.md`
- `agents/*.md`

Notes:
- `config.json` stores the OpenRouter API key and model settings locally.
- Profile and agent files are editable from the app UI and via your file explorer.

## Commands

- `npm run dev`: Vite web dev server only (no Tauri backend process)
- `npm run tauri dev`: full desktop app dev mode
- `npm run build`: frontend production build
- `npm run tauri build`: desktop production build/bundle
- `npm run test`: all frontend + backend tests
- `npm run test:all`: same as above
- `npm run test:frontend`: Vitest frontend suite
- `npm run test:frontend:watch`: Vitest watch mode
- `npm run test:frontend:unit`: frontend tests matching `unit_`
- `npm run test:frontend:integration`: frontend tests matching `integration_`
- `npm run test:frontend:e2e`: Playwright tests
- `npm run test:backend`: full Cargo test suite (`src-tauri/Cargo.toml`)
- `npm run test:backend:unit`: backend tests matching `unit_`
- `npm run test:backend:integration`: backend tests matching `integration_`
- `npm run test:backend:e2e`: backend tests matching `e2e_`

## Testing Notes

- Frontend integration/e2e tests mock Tauri `invoke` so they can run in browser test environments.
- Backend tests run against in-memory/temp SQLite and temp filesystem directories.

## Release Automation

Release binaries are built by GitHub Actions on published GitHub Releases:
- workflow: `.github/workflows/release.yml`
- trigger: `release.published`
- targets: Linux x64, Windows x64, macOS arm64, macOS x64

Current CI config does not include code signing/notarization.
