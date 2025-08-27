@probelabs/vow — minimal cross‑platform consent gate

What it does
- Enforces a simple consent gate based on two files at the repo root:
  - `AGENT_CONSENT.md` — your consent text shown on failure
  - `.AGENT_CONSENT` — ephemeral file; if missing, the CLI prints the markdown and exits 1
- Always removes `.AGENT_CONSENT` after each run to require fresh consent per attempt
- Works on macOS, Linux, and Windows (requires Node.js)

Install
- npm: `npm i -D @probelabs/vow`
- pnpm: `pnpm add -D @probelabs/vow`
- yarn: `yarn add -D @probelabs/vow`

Usage
- Git pre-commit (sh):
  - `vow-consent`  # exits 1 and prints AGENT_CONSENT.md if .AGENT_CONSENT is missing

- Claude Stop hook (command):
  - `vow-consent`  # same behavior as git hook

Notes
- If `AGENT_CONSENT.md` is not present, the tool is a no-op and exits 0.
- Keep `.githooks/*` with LF line endings for Git for Windows.

License
- MIT

