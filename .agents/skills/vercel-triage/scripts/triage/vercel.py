"""Read-only `vercel logs` transport: argv construction, JSON Lines parsing, paging, and error mapping."""

from __future__ import annotations

import dataclasses
import json
import shlex
import subprocess
from collections.abc import Callable
from typing import Any

from triage.constants import (
    BLOCKED_FIXES,
    GENERIC_FIX,
    PAGE_SIZE,
    PROJECT,
    SCOPE,
    SUBPROCESS_TIMEOUT_SECONDS,
    VERCEL_BIN,
)
from triage.redact import redact

Runner = Callable[[list[str]], subprocess.CompletedProcess[str]]

SOURCES = ("runtime", "requests")


@dataclasses.dataclass
class VercelError(Exception):
    reason: str
    message: str
    hint: str = ""
    next_commands: list[str] = dataclasses.field(default_factory=list)
    retryable: bool = False
    command: str = ""

    def blocked(self) -> dict[str, Any]:
        """Blocked result naming the operator fix. Never includes credentials."""
        return {
            "status": "blocked",
            "reason": self.reason,
            "message": self.message,
            "fix": BLOCKED_FIXES.get(self.reason, GENERIC_FIX),
            "hint": self.hint,
            "next": self.next_commands,
            "retryable": self.retryable,
            "command": self.command,
        }


def default_runner(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )


def query_argv(
    source: str,
    *,
    environment: str,
    since: str,
    until: str | None,
    deployment: str | None,
    query: str | None,
    limit: int,
) -> list[str]:
    argv = [
        VERCEL_BIN,
        "logs",
        source,
        "--scope",
        SCOPE,
        "--project",
        PROJECT,
        "--environment",
        environment,
        "--since",
        since,
    ]
    if until:
        argv += ["--until", until]
    if deployment:
        argv += ["--deployment", deployment]
    if query:
        argv += ["--query", query]
    return [*argv, "--json", "--limit", str(limit)]


def next_argv(source: str, cursor: str, limit: int) -> list[str]:
    # The cursor carries the team, target, filter, and window; the CLI rejects those flags beside --next. The
    # subcommand stays because bare `vercel logs` means `vercel logs runtime`.
    return [
        VERCEL_BIN,
        "logs",
        source,
        "--next",
        cursor,
        "--json",
        "--limit",
        str(limit),
    ]


def render_command(argv: list[str]) -> str:
    return shlex.join(argv)


def _json_lines(text: str) -> list[dict[str, Any]]:
    objects: list[dict[str, Any]] = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            objects.append(payload)
    return objects


def _error_from(payload: dict[str, Any], command: str) -> VercelError:
    next_commands = [
        str(item.get("command"))
        for item in payload.get("next") or []
        if isinstance(item, dict) and item.get("command")
    ]
    return VercelError(
        reason=str(payload.get("reason") or "cli_error"),
        message=redact(str(payload.get("message") or "")),
        hint=redact(str(payload.get("hint") or "")),
        next_commands=next_commands,
        retryable=bool(payload.get("retryable")),
        command=command,
    )


def run_page(
    runner: Runner, argv: list[str]
) -> tuple[list[dict[str, Any]], dict[str, Any], list[dict[str, Any]]]:
    """Run one `vercel logs --json` call and return (records, summary, warnings), or raise VercelError."""
    command = render_command(argv)
    try:
        proc = runner(argv)
    except FileNotFoundError:
        raise VercelError(
            "vercel_missing", f"`{VERCEL_BIN}` is not on PATH.", command=command
        ) from None
    except subprocess.TimeoutExpired:
        raise VercelError(
            "timeout",
            f"`vercel logs` exceeded {SUBPROCESS_TIMEOUT_SECONDS}s.",
            retryable=True,
            command=command,
        ) from None

    stdout_objects = _json_lines(proc.stdout or "")
    for payload in [*stdout_objects, *_json_lines(proc.stderr or "")]:
        if payload.get("status") == "error":
            raise _error_from(payload, command)

    records: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    summary: dict[str, Any] | None = None
    for payload in stdout_objects:
        kind = payload.get("type")
        if kind in ("log", "request") and isinstance(payload.get("data"), dict):
            records.append(payload["data"])
        elif kind == "summary":
            summary = payload
        elif kind == "warning":
            warnings.append(
                {
                    "reason": payload.get("reason"),
                    "message": redact(str(payload.get("message", ""))),
                }
            )

    if proc.returncode != 0 or summary is None:
        tail = redact((proc.stderr or proc.stdout or "").strip()[-500:])
        raise VercelError(
            "cli_failed",
            f"`vercel logs` exited {proc.returncode} without a JSON summary: {tail}",
            command=command,
        )
    return records, summary, warnings


def fetch_source(
    runner: Runner,
    source: str,
    *,
    first_argv: Callable[..., list[str]] | None,
    cursor: str | None,
    limit: int,
) -> dict[str, Any]:
    """Page one source up to `limit` records, following `pagination.next` from each summary."""
    records: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    commands: list[str] = []
    has_more = False
    next_cursor: str | None = None

    page_limit = min(limit, PAGE_SIZE)
    if cursor:
        argv = next_argv(source, cursor, page_limit)
    elif first_argv is not None:
        argv = first_argv(limit=page_limit)
    else:
        raise ValueError("fetch_source needs first_argv or cursor")

    while True:
        page, summary, page_warnings = run_page(runner, argv)
        commands.append(render_command(argv))
        records.extend(page)
        warnings.extend(page_warnings)
        pagination = (
            summary.get("pagination")
            if isinstance(summary.get("pagination"), dict)
            else {}
        )
        has_more = bool(pagination.get("hasMore"))
        next_cursor = (
            pagination.get("next") if isinstance(pagination.get("next"), str) else None
        )
        remaining = limit - len(records)
        if not has_more or not next_cursor or not page or remaining <= 0:
            break
        argv = next_argv(source, next_cursor, min(remaining, PAGE_SIZE))

    return {
        "source": source,
        "records": records[:limit],
        "recordCount": min(len(records), limit),
        "hasMore": has_more and bool(next_cursor),
        "next": next_cursor if has_more else None,
        "commands": commands,
        "warnings": warnings,
    }
