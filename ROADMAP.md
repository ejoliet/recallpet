# RecallPet — Roadmap (Phases 1–8)

> Committed for direction. Agents implement ONE phase at a time, each from its own phase prompt. Never build ahead. Preserve all prior-phase behavior.

## Recorded decisions

| # | Decision | Rationale | Trade-off accepted |
|---|---|---|---|
| D1 | Phase 0 stores clipboard text **raw, unencrypted, no redaction** | Owner's explicit call for spike velocity; local-only data | Backups/sync of home dir will carry secrets until Phase 4 |
| D2 | Browser capture via **Chrome MV3 extension**, not History.db reads | Live events, no Full Disk Access, no locked-file copy tricks | Chrome-only at first; extension maintenance; native messaging setup |
| D3 | LLM layer is **Ollama over a constrained tool API**, never WebLLM | WebGPU in WKWebView unreliable; Ollama already runs locally (AskLocal) | Requires Ollama installed; feature is optional |
| D4 | Phase 6 (third-party request observation) is a **separate opt-in install moment** | It needs broad host permissions (`<all_urls>` + webRequest) that Phase 2 visit capture must not request | Two permission prompts instead of one |

## Target architecture

```
┌────────────────────────────────────────────┐
│ RecallPet (Tauri v2)                       │
│ Rust core                                  │
│ ├── Clipboard collector          (P0)      │
│ ├── Shell-event receiver         (P1)      │
│ ├── Browser-event receiver       (P2)      │
│ ├── Unified events + analytics   (P3)      │
│ ├── Redaction/retention/excl.    (P4)      │
│ ├── Optional Ollama client       (P5)      │
│ └── SQLite + FTS5                          │
│ Webview: avatar · timeline · search ·      │
│ stats · privacy controls · NL query        │
└────────────────────────────────────────────┘
        ▲                    ▲
   zsh hooks (P1)     Chrome extension (P2, P6)
```

## Phase 1 — Shell command capture

- zsh `preexec` (command, start) + `precmd` (exit status, duration).
- Transport: Unix domain socket primary; JSON Lines spool file fallback when app is down.
- Store: command, executable, cwd, exit status, duration, timestamp, terminal app, optional git repo/branch.
- Exclusions: directories, commands, executables, leading-space commands.
- Never: read history files, capture output, capture env values, execute commands, global key monitoring.
- Redaction of secret-bearing arguments: deferred to Phase 4 (`AIDEV-TODO` at socket/spool write points). Raw until then per D1.
- **Done when**: commands appear in timeline with correct status/duration; integration fully uninstallable.

## Phase 2 — Browser visit capture (Chrome MV3 extension)

- Capture: top-level navigation, title, normalized URL, domain, timestamp, transition type.
- Defaults: strip query strings, ignore Incognito, no page text/forms/cookies/bodies, **no broad host permissions** (D4).
- Transport: Native Messaging for packaged product; authenticated loopback HTTP (127.0.0.1, random token, schema-validated, rate-limited) for dev only.
- **Done when**: visits stream live, duplicate navigations suppressed, extension removal leaves core app intact.

## Phase 3 — Unified timeline + deterministic analytics

- One normalized `events` model over clipboard/commands/visits; source-specific tables for structured fields.
- Filters, date ranges, search, saved searches, export JSON/CSV, per-source and date-range deletion.
- Deterministic stats before any LLM: top reused clips, frequent commands, highest-failure commands, top domains, visits by hour/weekday, repo activity, storage growth.
- Every statistic links back to source records. Browser duration is an estimate — never claim exact attention time.

## Phase 4 — Privacy, redaction, retention

The safety debt from D1 is paid here:

- Deterministic secret detection (AWS keys, GitHub tokens, JWTs, PEM blocks, password/API-key assignments, connection strings, high-entropy strings) with **negative tests** so git SHAs, UUIDs, and long URLs are NOT flagged.
- Redact before persistence, spool files, logs, and (later) Ollama.
- Retroactive scan-and-redact/delete over existing raw Phase 0–3 data.
- Global pause, pause-15-min, per-collector toggles, app/domain/directory exclusions, configurable retention, visible collection indicator. No hidden collection.

## Phase 5 — Optional Ollama integration

- App fully useful without it. Never part of install success criteria.
- Constrained read-only tool layer (`search_events`, `top_domains`, `failed_commands`, `clipboard_reuse`, `visit_timeline`, `events_around_time`); Rust validates args and runs parameterized SQL; Ollama summarizes only returned records; UI shows source evidence.
- No raw SQL tool, no filesystem access, temperature ~0, strict schemas, clear "AI unavailable" state, never start/stop Ollama silently, never remote APIs.

## Phase 6 — Third-party connection observation (opt-in, D4)

- Only after Phase 2 is stable. Separate permission grant, independently disableable.
- Capture only: first-party domain, third-party domain, request count/type, first/last seen, deterministic classification (analytics/ads/CDN/media/auth/API/unknown).
- Never: headers, cookies, bodies, form values, full query strings, page content. Never label "malicious" without a verified source.

## Phase 7 — Dashboard

- Heatmaps, reuse histograms, failure trends, top domains, first-vs-third-party ratio, storage growth. Canvas or small chart lib. Every viz drills down to source events. No large framework unless maintenance genuinely demands it.

## Phase 8 — Packaging and hardening

- Signing, notarization, hardened runtime, Launch at Login, Native Messaging installer, safe uninstall, migration rollback, DB backup/corruption recovery, permission disclosures, privacy policy, extension permission audit, perf/power/storage profiling. Unpacked extension during validation; Web Store submission last.

## Standing exclusions

Electron, React, cloud DB, remote vector DB, external AI APIs, WebLLM in the webview, History.db as primary browser integration, model-generated raw SQL, continuous avatar animation, team/employee surveillance, cross-device sync, mobile.

## Milestone gate (every phase)

1. Automated tests pass. 2. Manual validation recorded by a human. 3. Privacy behavior documented. 4. Failure modes documented. 5. Uninstall documented. 6. Migrations tested. 7. All prior phases still work. 8. From Phase 4 on: no raw captured data in logs.

Update `HANDOFF.md` after each phase: features done, files changed, schema changes, permissions introduced, tests added, known defects, privacy implications, exact next milestone, explicit non-goals. Decisions live in this file and in code `AIDEV-NOTE`s — never only in chat.
