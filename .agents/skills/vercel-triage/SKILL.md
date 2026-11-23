---
argument-hint: "[--since 7d] [--until <time>] [--deployment <id-or-url>] [--limit N] [report-only]"
compatibility: Requires uv and the Vercel CLI (65.x) logged in to the sablier team, or VERCEL_TOKEN with sablier access.
coordination: exempt
name: vercel-triage
description:
  Triage merkle-api production failures from Vercel runtime logs and CDN request records, the service's only telemetry
  (no Sentry). Group and classify errors as actionable, upstream, platform, or expected, fix first-party Rust bugs with
  tests, and report. Use for merkle-api incidents, 5xx spikes, Pinata/IPFS failures, or a routine production log review.
---

# Vercel Triage

Triage `sablier/merkle-api` production logs with the bundled read-only helper. It reads Vercel runtime logs and CDN
request records, groups them by signature, and classifies each group against the Rust code's structured log events and
status paths. In manual mode, fix the first-party causes in code.

This skill is coordination-exempt: skip the ai-coord gate for its declared work. The exemption covers only fetching and
triage, which read Vercel and change nothing. The Fix phase edits the repository and re-enters the ai-coord gate.

## Authority

- Explicit user instructions take precedence over the defaults in this skill. When you set a default aside, name the
  rule you set aside in the response and keep the write boundary below.
- The helper is read-only. It runs only `vercel logs` with an argv list, never a shell, and changes no repository files
  or Vercel state. Never run `vercel deploy`, `promote`, `env`, `rollback`, or `login` on the user's behalf.
- In manual mode, the Fix phase is the default and authorizes repository edits and commits for the triaged groups. Skip
  it when the user asks for a report only.
- Deploy only when the user explicitly asks. Then follow the manual `Deploy on Vercel` workflow in `AGENTS.md`
  (`gh workflow run`); it runs CI, smoke probes, and promotion. Otherwise report that the fix awaits deploy.
- Vercel has no issue state, so there is no status or apply mode.

## Invocations

```text
/vercel-triage [--since 7d] [--until <time>] [--deployment <id-or-url>] [--limit N] [report-only]
```

A bare `/vercel-triage` reviews the last 7 days of production logs, investigates actionable groups, and runs the Fix
phase. Pass `--since`/`--until` only when the user names a window (ISO 8601 or relative such as `24h`, `3d`). Pass
`--deployment` to scope to one deployment, for example after a release. Vercel's log retention depends on the plan, so a
window older than retention returns fewer records rather than an error.

## Setup

```bash
TRIAGE_PY=".agents/skills/vercel-triage/scripts/triage.py"
uv run --script "$TRIAGE_PY" check
```

`check` probes `vercel logs` for scope `sablier`, project `merkle-api`, environment `production`. It prints
`{"status": "ok", ...}` and exits 0, or prints a blocked object and exits 3. The blocked object carries the CLI's
`reason`, `message`, `hint`, `next` commands, and a `fix`:

| `reason`                                                 | Fix                                                                     |
| -------------------------------------------------------- | ----------------------------------------------------------------------- |
| `login_required`, `unauthorized`                         | The user runs `vercel login --scope sablier`, or exports `VERCEL_TOKEN` |
| `forbidden`, `permission_denied`, `scope_not_accessible` | Log in as a sablier member, or use a token scoped to the sablier team   |
| `not_linked`, `not_found`, `project_not_found`           | Verify the scope and project; the project ID is in `AGENTS.md`          |
| `rate_limited`, `network_error`, `timeout`               | Retry once later                                                        |
| `vercel_missing`                                         | Install the Vercel CLI                                                  |

When blocked, stop and return the report sections below with the blocker and its fix in `## Findings Summary`. Login
needs a person to approve it in a browser, so never start it yourself. Never print `VERCEL_TOKEN` or any header.

## Triage

### 1. Fetch and Classify

```bash
TRIAGE_JSON="$(mktemp)"
TRIAGE_MD="$(mktemp)"
uv run --script "$TRIAGE_PY" triage --since 7d > "$TRIAGE_JSON"
uv run --script "$TRIAGE_PY" report-md --triage "$TRIAGE_JSON" > "$TRIAGE_MD"
```

`triage` always passes `--scope sablier --project merkle-api` and `--environment production` (override with
`--environment preview` only when the user asks). It fetches two sources:

| Source     | Default `--query`                                    | Override           |
| ---------- | ---------------------------------------------------- | ------------------ |
| `runtime`  | `level:(error OR fatal OR warning) OR stream:stderr` | `--runtime-query`  |
| `requests` | `httpStatus >= 500`                                  | `--requests-query` |

Each source pages through the CLI's `pagination.next` cursor up to `--limit` records (default 2000). When the JSON shows
`sources.<name>.hasMore`, raise `--limit` or continue that source alone with
`--source <name> --next <sources.<name>.next>`; the cursor keeps the original filter and window. A blocked result
(exit 3) has the same shape as `check`. A failing `requests` source with a non-auth reason, such as a plan that lacks
request records, becomes a warning and triage continues with runtime groups only.

`eligibility_slow` is logged to stdout at level `warn`. If Vercel indexes it as `info`, the default runtime query misses
it; add it with `--runtime-query 'level:(error OR fatal OR warning) OR stream:stderr OR eligibility_slow'` when latency
is in scope.

The JSON holds `query`, `sources` (record counts, paging, and the exact commands run), `warnings`, `categoryCounts`, and
`groups`. Each group has `signature`, `category`, `reason`, `endpoints`, `count`, `firstSeen`, `lastSeen`,
`httpStatuses`, `deploymentIds`, `sampleRequestIds`, `maxElapsedMs`, and one redacted `sampleMessage`. Request paths
lose their query strings, and token-like values, bearer headers, and JWTs are masked.

### 2. Signatures and Categories

Structured events come from `utils::logging::log_event` (one JSON line with `event` and `level`). Their signature is
`<event>:<error_kind>[/<provider_status>]@<endpoint>`. Other messages get `<kind>:<normalized message>@<endpoint>`, with
CIDs, `0x` values, hex, UUIDs, request IDs, durations, and numbers stripped. A CDN 5xx record whose request ID matches a
fetched runtime entry is counted in that group's `httpStatuses`; the rest form `http_<status>@<endpoint>` groups.

| Category     | Signals                                                                                                                                                                                                                                                                                                                           |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `actionable` | Rust panics (including `expect` on missing env vars), `secrets_loader` startup errors (`Error: MissingKey(...)` etc.), `eligibility_malformed_tree`, `pinata_upload_parse_error`, process exits, payload-too-large, `eligibility_ipfs_error` kinds `invalid_cid`/`not_found`/`deserialize`, unexplained 5xx, and anything unknown |
| `upstream`   | `eligibility_ipfs_error` kinds `request_timeout`, `request_connect`, `request_decode`, `request_error`, `upstream`; `pinata_upload_error`; unlogged validity 502/504                                                                                                                                                              |
| `platform`   | Invocation timeouts, OOM (`signal: killed`), init timeouts, `INTERNAL_FUNCTION_*`                                                                                                                                                                                                                                                 |
| `expected`   | `eligibility_slow` (over the 2 s threshold), and, when a custom requests query fetches them, 401 bearer rejections, eligibility 400 not-eligible, validity 400 bad CID, other 4xx                                                                                                                                                 |

The classifier is conservative: anything it does not recognize is `actionable`. `invalid_cid`/`not_found`/`deserialize`
stay actionable because the cause is usually the caller's CID or the pinned content, which needs a decision rather than
a filter. When you add a structured event in Rust, add its rule to `scripts/triage/classify.py` and a case to
`scripts/test_triage.py`.

### 3. Investigate Actionable Groups

- Pull the full entries for a group with its sample request ID; the report's `<details>` block has the command.
- Map the signature to code: structured events to their `log_event` call sites under `src/controller/`, panics to the
  file in the signature, `secrets_loader` to `src/utils/secrets.rs` and the encrypted `.env.production`.
- Compare `deploymentIds` and `firstSeen` with `git log` for the implicated files. A later commit may already fix it, or
  a deploy may have introduced it.
- For `invalid_cid`/`not_found`, check which CIDs fail and whether the caller (merkle-tracker or the portal) produced
  them before touching Rust.
- Keep conclusions grounded in log entries and code paths. Do not edit code before the Fix phase.

### 4. Fix

Run this phase in manual mode unless the user asked for a report only. Choose one outcome per group:

| Classification         | Outcome                                                                                                                                  |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `actionable`, real bug | Fix the root cause with a colocated `#[cfg(test)]` test that reproduces it (mock HTTP through the shared `SERVER`).                      |
| `actionable`, caller   | Change nothing here. Point at the owner: `/Users/prb/sablier/backend/services` (merkle-tracker) or `/Users/prb/sablier/new-ui` (portal). |
| `actionable`, unclear  | Change nothing. Report the missing evidence and the next diagnostic query.                                                               |
| `upstream`             | Change nothing unless a first-party timeout, retry, or status mapping is wrong. Report Pinata/gateway incidents with counts and windows. |
| `platform`             | Change nothing unless our configuration causes it, such as a function maxDuration below the 12 s IPFS timeout.                           |
| `expected`             | Change nothing.                                                                                                                          |

Preserve `AGENTS.md`'s Production App Compatibility contract in every fix:

- Never map provider failures, malformed input, or invalid CIDs to a sub-500 status; Railway treats sub-500 as an
  eligibility verdict.
- Keep eligibility `400` meaning "not eligible", with its body.
- Keep `create`'s 200-on-bad-input quirk in the Vercel adapter.
- Keep the eligibility `Cache-Control: public, s-maxage=31536000, immutable` header.
- Keep `is_authorized` fail-closed, and never log tokens or headers.

Combine groups that share one root cause into one change. Before each edit, acquire its scopes through the ai-coord
gate. Validate each change with:

```bash
cargo +nightly fmt --all -- --check
cargo +stable clippy --all-targets -- -D warnings
cargo +stable test --locked
```

Commit each coherent fix with `$commit` before starting the next. Deploy only under Authority.

### 5. Final Response

1. `## Findings Summary`: the rendered category tables from `$TRIAGE_MD` (`Actionable`, `Upstream`, `Platform`,
   `Expected`) with the window, record counts, and any paging or source warnings. If blocked, explain the blocker and
   its fix here.
2. `## Suggested Fixes`: for each actionable group, the likely root cause, the relevant file paths, and whether it is a
   first-party bug, a caller problem, or unclear. Separate verified causes from hypotheses. Report each applied fix with
   its commit and the checks that passed, and say whether it awaits deploy. Add a snippet only for groups left unfixed.
   If there are no actionable groups, say that no code changes are needed.
3. A `<details>` block with the exact `vercel logs` commands from `$TRIAGE_MD`: each source's fetch and each group's
   sample-request query.

Return Markdown directly. If triage is blocked or fails, still return the three sections.

## Completion

Finish when the window has been triaged or reported blocked, every classification claim is grounded in the current
helper output, and every actionable group in manual mode is fixed and committed or reported with its owner or blocker.
