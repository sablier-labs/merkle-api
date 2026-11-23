from __future__ import annotations

import re

VERCEL_BIN = "vercel"
SCOPE = "sablier"
PROJECT = "merkle-api"
ENVIRONMENT_DEFAULT = "production"
SINCE_DEFAULT = "7d"
LIMIT_DEFAULT = 2000
LIMIT_MAX = 100_000
PAGE_SIZE = 1000
SUBPROCESS_TIMEOUT_SECONDS = 300

RUNTIME_QUERY_DEFAULT = "level:(error OR fatal OR warning) OR stream:stderr"
REQUESTS_QUERY_DEFAULT = "httpStatus >= 500"

CATEGORIES = ("actionable", "upstream", "platform", "expected")

EXIT_BLOCKED = 3

# `vercel logs --json` error reasons, mapped to the operator fix.
LOGIN_FIX = (
    "Run `vercel login --scope sablier` (a person approves it in a browser), or export VERCEL_TOKEN for an "
    "account with access to the sablier team, then rerun `check`."
)
ACCESS_FIX = (
    "The authenticated account or VERCEL_TOKEN cannot read sablier/merkle-api logs. Log in as a sablier team member "
    "with `vercel login --scope sablier`, or use a VERCEL_TOKEN scoped to the sablier team."
)
PROJECT_FIX = (
    "Vercel could not resolve scope `sablier` / project `merkle-api` (prj_a6lRAog7uUzq5mXCMsuVYGw3hIEM). Verify the "
    "team and project names with the account owner."
)
TRANSIENT_FIX = "Transient Vercel API failure. Retry once later; if it persists, report the CLI message."
GENERIC_FIX = "Read the CLI message and hint, fix the cause, then rerun `check`."

BLOCKED_FIXES = {
    "login_required": LOGIN_FIX,
    "unauthorized": LOGIN_FIX,
    "forbidden": ACCESS_FIX,
    "permission_denied": ACCESS_FIX,
    "scope_not_accessible": ACCESS_FIX,
    "missing_scope": ACCESS_FIX,
    "not_linked": PROJECT_FIX,
    "not_found": PROJECT_FIX,
    "project_not_found": PROJECT_FIX,
    "rate_limited": TRANSIENT_FIX,
    "network_error": TRANSIENT_FIX,
    "timeout": TRANSIENT_FIX,
    "vercel_missing": "Install the Vercel CLI (`bun add -g vercel` or `npm i -g vercel`) and rerun `check`.",
}

# Reasons that make every source fail, so triage stops instead of reporting a partial result.
FATAL_REASONS = frozenset(
    {
        "login_required",
        "unauthorized",
        "forbidden",
        "permission_denied",
        "scope_not_accessible",
        "missing_scope",
        "not_linked",
        "not_found",
        "project_not_found",
        "vercel_missing",
    }
)

# Structured events from src/utils/logging.rs callers. Keep in sync with the Rust `log_event` call sites.
IPFS_UPSTREAM_KINDS = frozenset(
    {
        "request_timeout",
        "request_connect",
        "request_decode",
        "request_error",
        "upstream",
    }
)
IPFS_INPUT_KINDS = frozenset({"invalid_cid", "not_found", "deserialize"})

# Debug names of `utils::secrets::SecretsError`; `main` returning Err prints `Error: <Debug>` and exits 1.
SECRETS_ERROR_PATTERN = re.compile(
    r"\bError: (MissingPrivateKey|InvalidPrivateKey|InvalidLine|PrivateKeyInFile|Unencrypted|Decrypt|MissingKey)\b"
    r"(?:\(\"?([A-Z0-9_]*)\"?\))?"
)
PANIC_PATTERN = re.compile(r"panicked at ([^\s:]+)(?::\d+:\d+)?:?\s*(.*)", re.DOTALL)

FUNCTION_TIMEOUT_PATTERN = re.compile(
    r"Task timed out after|FUNCTION_INVOCATION_TIMEOUT|Vercel Runtime Timeout Error",
    re.IGNORECASE,
)
OOM_PATTERN = re.compile(
    r"signal: killed|out of memory|Runtime\.OutOfMemory|memory size exceeded",
    re.IGNORECASE,
)
INIT_TIMEOUT_PATTERN = re.compile(
    r"INIT_REPORT.*Status:\s*timeout|Init(?:ialization)? timed out", re.IGNORECASE
)
PLATFORM_INTERNAL_PATTERN = re.compile(
    r"\bINTERNAL_FUNCTION_[A-Z_]+|\bINTERNAL_EDGE_FUNCTION_[A-Z_]+"
)
PROCESS_EXIT_PATTERN = re.compile(
    r"Runtime exited with error: exit status|Runtime\.ExitError|INIT_REPORT.*Status:\s*error|"
    r"FUNCTION_INVOCATION_FAILED|NO_RESPONSE_FROM_FUNCTION",
    re.IGNORECASE,
)
PAYLOAD_TOO_LARGE_PATTERN = re.compile(
    r"FUNCTION_(?:RESPONSE_)?PAYLOAD_TOO_LARGE|payload too large", re.IGNORECASE
)
