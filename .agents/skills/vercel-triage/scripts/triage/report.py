"""Markdown rendering of triage JSON."""

from __future__ import annotations

import re
from typing import Any

from triage.constants import CATEGORIES
from triage.vercel import query_argv, render_command

TITLES = {
    "actionable": "Actionable",
    "upstream": "Upstream",
    "platform": "Platform",
    "expected": "Expected",
}


def escape_cell(value: Any) -> str:
    return re.sub(r"\r?\n", " ", str(value)).replace("|", "\\|")


def _counts(mapping: dict[str, int]) -> str:
    return ", ".join(f"{key}×{value}" for key, value in mapping.items()) or "-"


def _code(value: Any) -> str:
    """Inline code span; a backtick in the value would end the span early."""
    return f"`{escape_cell(value).replace('`', "'")}`"


def _codes(values: list[str]) -> str:
    return ", ".join(_code(value) for value in values) or "-"


def group_command(group: dict[str, Any], query: dict[str, Any]) -> str | None:
    """`vercel logs` command that reproduces one group's sample request in the triage window."""
    request_ids = group.get("sampleRequestIds") or []
    if not request_ids:
        return None
    source = "requests" if group.get("source") == "requests" else "runtime"
    return render_command(
        query_argv(
            source,
            environment=query.get("environment", "production"),
            since=query.get("since", "7d"),
            until=query.get("until"),
            deployment=query.get("deployment"),
            query=f'requestId:"{request_ids[0]}"',
            limit=100,
        )
    )


def _table(lines: list[str], groups: list[dict[str, Any]]) -> None:
    lines.append(
        "| Signature | Endpoints | Count | HTTP | First Seen | Last Seen | Request IDs | Notes |"
    )
    lines.append(
        "|-----------|-----------|-------|------|------------|-----------|-------------|-------|"
    )
    if not groups:
        lines.append("| None | - | - | - | - | - | - | - |")
        return
    for group in groups:
        notes = group.get("reason", "")
        if group.get("maxElapsedMs") is not None:
            notes = f"{notes} Max elapsed {group['maxElapsedMs']} ms."
        if group.get("deploymentCount"):
            notes = f"{notes} Deployments: {group['deploymentCount']}."
        lines.append(
            f"| {_code(group.get('signature', ''))} | {escape_cell(', '.join(group.get('endpoints') or []))} "
            f"| {group.get('count', 0)} | {escape_cell(_counts(group.get('httpStatuses') or {}))} "
            f"| {escape_cell(group.get('firstSeen') or '-')} | {escape_cell(group.get('lastSeen') or '-')} "
            f"| {_codes((group.get('sampleRequestIds') or [])[:2])} | {escape_cell(notes)} |"
        )


def render_report(triage: dict[str, Any]) -> str:
    query = triage.get("query") or {}
    sources = triage.get("sources") or {}
    groups = [group for group in triage.get("groups") or [] if isinstance(group, dict)]

    lines = ["# Vercel Triage Report", ""]
    lines.append(
        f"**Project:** `{query.get('scope', 'sablier')}/{query.get('project', 'merkle-api')}`"
    )
    lines.append(f"**Environment:** {query.get('environment', 'production')}")
    window = f"since {query.get('since', '-')}" + (
        f" until {query['until']}" if query.get("until") else " until now"
    )
    lines.append(f"**Window:** {window}")
    if query.get("deployment"):
        lines.append(f"**Deployment:** `{query['deployment']}`")
    for name, info in sources.items():
        if not isinstance(info, dict):
            continue
        if info.get("error"):
            lines.append(
                f"**{name} records:** unavailable ({escape_cell(info['error'].get('reason', 'error'))})"
            )
            continue
        more = (
            " (more available; raise --limit or continue with --next)"
            if info.get("hasMore")
            else ""
        )
        lines.append(f"**{name} records:** {info.get('recordCount', 0)}{more}")
    lines.append(f"**Groups:** {len(groups)}")
    for warning in triage.get("warnings") or []:
        lines.append(f"> Warning: {escape_cell(warning)}")
    lines.append("")

    lines.append("## Findings Summary")
    for category in CATEGORIES:
        rows = [group for group in groups if group.get("category") == category]
        lines += ["", f"### {TITLES[category]} ({len(rows)} groups)", ""]
        _table(lines, rows)
    lines.append("")

    lines += ["## Suggested Fixes", ""]
    actionable = [group for group in groups if group.get("category") == "actionable"]
    if actionable:
        for group in actionable:
            lines.append(
                f"- {_code(group['signature'])} ({group.get('count', 0)}×): {escape_cell(group.get('reason', ''))}"
            )
            if group.get("sampleMessage"):
                lines.append(f"  - Sample: {_code(group['sampleMessage'])}")
    else:
        lines.append("No actionable groups; no code changes are needed.")
    lines.append("")

    lines += ["<details>", "<summary>Reproduce with vercel logs</summary>", ""]
    commands: list[str] = []
    for name, info in sources.items():
        if isinstance(info, dict):
            commands += [f"# {name} fetch", *(info.get("commands") or [])]
            if info.get("hasMore") and info.get("next"):
                commands.append(
                    f"# {name} continuation: triage --source {name} --next <cursor from triage JSON>"
                )
    for group in groups:
        command = group_command(group, query)
        if command:
            commands += [f"# {group['category']}: {group['signature']}", command]
    if commands:
        lines += ["```bash", *commands, "```"]
    else:
        lines.append("No commands recorded.")
    lines += ["", "</details>"]
    return "\n".join(lines) + "\n"
