# Sablier Merkle API

Private Rust backend for creating and verifying Merkle trees used by Sablier airdrops. Deployed as Vercel Lambdas. GPL-3.0 source published for transparency; not intended for third-party self-hosting.

@README.md

## Stack

- Rust 2021 edition, nightly toolchain (CI pins `dtolnay/rust-toolchain@nightly`)
- `vercel_runtime` 2 — each endpoint is its own Lambda binary
- `merkle-tree-rs` (OpenZeppelin-compatible `StandardMerkleTree`) for EVM
- `solana-sdk` + `bs58` + custom `utils::solana_merkle` for Solana
- `ethers-rs` for EIP-55 address handling, `sha3` for Keccak
- `reqwest` + `multipart` for Pinata IPFS uploads/downloads
- `csv` + `regex` for campaign parsing, validation
- `tokio` async runtime, `mockito` for HTTP mocking in tests

## Layout

- `api/*.rs` — thin Vercel Lambda `main` shims; one binary per endpoint (see `[[bin]]` table in `Cargo.toml`).
- `src/` — `sablier_merkle_api` library crate.
  - `controller/` — request handlers. Each exposes `handler` (generic, testable) and `handler_to_vercel` (Vercel adapter).
  - `services/ipfs.rs` — Pinata upload + IPFS gateway download. All errors funnel through `IpfsError`.
  - `utils/` — `auth` (bearer check), `csv_validator`, `request` (query parsing), `solana_merkle`.
  - `csv_campaign_parser.rs` — `CampaignCsvParsed::build_ethereum` / `build_solana`.
  - `data_objects/` — `dto`, `query_param`, `response`.

## Endpoints

| Binary             | Auth   | Purpose                                       |
| ------------------ | ------ | --------------------------------------------- |
| `create`           | Bearer | Build EVM Merkle tree from CSV, pin to IPFS   |
| `create_solana`    | Bearer | Same, Solana addresses                        |
| `validity`         | Bearer | Verify an existing tree by CID                |
| `eligibility`      | Public | Fetch proof for `(cid, address)` — EVM        |
| `eligibility_solana` | Public | Same, Solana                                |
| `health`           | Public | Liveness probe                                |

Eligibility responses set `Cache-Control: public, s-maxage=31536000, immutable` — CIDs are content-addressed, so Vercel's edge cache replaces the old Redis layer. Do not weaken this without replacing the caching story.

## Commands

- `cargo fmt --all -- --check` — formatting gate (CI enforces)
- `cargo clippy --all-targets -- -D warnings` — lints (CI enforces, warnings denied)
- `cargo test` — unit tests; some tests hit `SERVER` mutex in `utils::async_test` and must share env setup
- `cargo build --release` — local build; real deploy cross-compiles via `cargo zigbuild --target x86_64-unknown-linux-gnu`

Deploy is manual via `Deploy on Vercel` workflow (`workflow_dispatch`). Do not add automatic deploys on push.

## Code Style

- `rustfmt.toml`: `max_width = 120`, `imports_granularity = "Crate"`, `use_small_heuristics = "Max"`, `tab_spaces = 4`, `wrap_comments = true`. Always `cargo fmt` before proposing changes.
- Prefer `let ... else` early returns over nested `match`/`if let` — existing controllers use this pattern consistently.
- Keep the `handler` / `handler_to_vercel` split: business logic lives in the pure `handler` so tests can call it directly without a `Vercel::Request`.
- Error responses go through `data_objects::response::{message, bad_request, ok, to_vercel, to_vercel_message}` — do not hand-roll JSON responses.
- Do not introduce `unwrap()` / `expect()` on external input paths. Internal invariants (e.g., `serde_json::to_string` on a tree we just built) are acceptable.
- Doc comments (`///`) on public items. Inline `//` comments only for non-obvious invariants — do not narrate what the code does.

## Auth & Secrets

- Protected endpoints call `utils::auth::is_authorized`. It is **fail-closed**: missing or empty `MERKLE_API_BEARER_TOKEN` rejects every request. Preserve this property — never fall back to "allow when unconfigured".
- Expected header is exact match: `Authorization: Bearer <MERKLE_API_BEARER_TOKEN>`. No scheme variations.
- Never log bearer tokens, Pinata keys, or full request headers.
- Required env vars (see `.env.example`): `PINATA_ACCESS_TOKEN`, `PINATA_API_KEY`, `PINATA_SECRET_API_KEY`, `PINATA_API_SERVER`, `IPFS_GATEWAY`, `MERKLE_API_BEARER_TOKEN`. Deploy-only: `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID`, `VERCEL_TOKEN`.

## Known Quirks

- `create` / `create_solana` return HTTP **200** for malformed input (missing `decimals`, bad content-type, unreadable body). This is intentional legacy behavior to preserve client compatibility — there is a `Review candidate` comment marking it. Do not "fix" to 4xx without coordinating with the frontend team.
- `handler` (pure) returns proper status codes (400/500). The 200-on-bad-input quirk only exists in the Vercel adapter.
- Mockito tests share a single `SERVER` mutex (`src/utils.rs`) on port 8000. New HTTP-facing tests must `lock().await` that server and `setup_env_vars` to avoid races.
- `StandardMerkleTree::of` leaves are `[index, address, amount]` typed as `[uint, address, uint256]` — matches the on-chain `MerkleLockup` / `MerkleLL` verifier layout. Don't reorder.

## Testing

- Put unit tests in `#[cfg(test)] mod tests` inside the controller/service they exercise — matches existing layout.
- For IPFS paths, mock Pinata via `mockito` using the shared `SERVER` from `utils::async_test`.
- Doc-tests in `csv_campaign_parser.rs` are real tests — keep them passing.

## Pull Requests

- Keep changes scoped. CSV parsing, Merkle construction, and IPFS are load-bearing — touch them only with a reason in the PR description.
- Run `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` locally; CI will fail otherwise.
- If you add an endpoint: new `api/<name>.rs` Lambda shim, new `[[bin]]` entry in `Cargo.toml`, new `src/controller/<name>.rs`, and register it in `src/controller.rs`.
