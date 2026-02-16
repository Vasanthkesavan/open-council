# Decision Copilot

Decision Copilot is a desktop app built with Tauri, React, TypeScript, and Rust.

It helps users make better decisions by:
- keeping conversation history
- maintaining local markdown profile files over time
- supporting both cloud and local LLM providers:
  - Anthropic API
  - Ollama (local models)

## Tech Stack

- Frontend: React 19 + Vite + TypeScript + Tailwind CSS
- Desktop shell/backend: Tauri 2 + Rust
- Storage: SQLite (local)

## Prerequisites

- Node.js 20+ and npm
- Rust (stable) and Cargo
- Tauri system prerequisites for your OS (WebView2 on Windows)

Optional (if using local models):
- Ollama installed and running

## Local Development

1. Install dependencies:

```bash
npm install
```

2. Start the desktop app in dev mode:

```bash
npm run tauri dev
```

This starts:
- Vite dev server on `http://localhost:1420`
- the Tauri desktop app window

Note: port `1420` is fixed (`strictPort: true`), so make sure nothing else is using it.

## First Run Setup

When the app opens, go to Settings and pick a provider.

### Anthropic

- Select `Anthropic API`
- Add your Anthropic API key
- Choose a model (default: `claude-sonnet-4-5-20250929`)

### Ollama

1. Start Ollama
2. Pull a model, for example:

```bash
ollama pull llama3.1:8b
```

3. In app Settings:
- Select `Ollama (Local)`
- Keep URL as `http://localhost:11434` (or set your custom endpoint)
- Set your model name

## Useful Scripts

- `npm run dev` - starts Vite only (web UI, no Tauri backend)
- `npm run tauri dev` - full desktop development mode
- `npm run build` - frontend production build
- `npm run tauri build` - desktop production build/bundles
- `npm run test:frontend:unit` - frontend unit tests (Vitest)
- `npm run test:frontend:integration` - frontend integration tests (Vitest)
- `npm run test:frontend:e2e` - frontend e2e tests (Playwright)
- `npm run test:backend:unit` - backend unit tests (Cargo)
- `npm run test:backend:integration` - backend integration tests (Cargo)
- `npm run test:backend:e2e` - backend e2e tests (Cargo)
- `npm run test:all` - run frontend + backend test suites

## Local App Data

On Windows, app data is stored under:

`%APPDATA%\com.decisioncopilot.app\`

Important files:
- `database.sqlite` - conversations and messages
- `config.json` - provider and model settings
- `profile\*.md` - user profile memory files
