from __future__ import annotations

import argparse
import datetime as dt
import functools
import json
import sys
from pathlib import Path
from typing import Any

from triage.constants import (
    CATEGORIES,
    ENVIRONMENT_DEFAULT,
    EXIT_BLOCKED,
    FATAL_REASONS,
    LIMIT_DEFAULT,
    LIMIT_MAX,
    PROJECT,
    REQUESTS_QUERY_DEFAULT,
    RUNTIME_QUERY_DEFAULT,
    SCOPE,
    SINCE_DEFAULT,
)
from triage.groups import build_groups
from triage.report import render_report
from triage.vercel import (
    SOURCES,
    Runner,
    VercelError,
    default_runner,
    fetch_source,
    query_argv,
    run_page,
)


def emit(payload: Any) -> None:
    print(json.dumps(payload, indent=2))


def positive_limit(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError:
        raise argparse.ArgumentTypeError("must be an integer") from None
    if not 1 <= value <= LIMIT_MAX:
        raise argparse.ArgumentTypeError(f"must be between 1 and {LIMIT_MAX}")
    return value


def cmd_check(args: argparse.Namespace, runner: Runner) -> int:
    argv = query_argv(
        "runtime",
        environment=args.environment,
        since="5m",
        until=None,
        deployment=None,
        query=None,
        limit=1,
    )
    try:
        run_page(runner, argv)
    except VercelError as error:
        emit(error.blocked())
        return EXIT_BLOCKED
    emit(
        {
            "status": "ok",
            "scope": SCOPE,
            "project": PROJECT,
            "environment": args.environment,
        }
    )
    return 0


def cmd_triage(args: argparse.Namespace, runner: Runner) -> int:
    if args.next and args.source == "all":
        print(
            "Error: --next continues one source; pass --source runtime or --source requests.",
            file=sys.stderr,
        )
        return 2

    queries = {"runtime": args.runtime_query, "requests": args.requests_query}
    selected = SOURCES if args.source == "all" else (args.source,)
    sources: dict[str, Any] = {}
    records: dict[str, list[dict[str, Any]]] = {name: [] for name in SOURCES}
    warnings: list[str] = []

    for name in selected:
        first_argv = functools.partial(
            query_argv,
            name,
            environment=args.environment,
            since=args.since,
            until=args.until,
            deployment=args.deployment,
            query=queries[name],
        )
        try:
            fetched = fetch_source(
                runner, name, first_argv=first_argv, cursor=args.next, limit=args.limit
            )
        except VercelError as error:
            if error.reason in FATAL_REASONS or name == selected[0]:
                emit(error.blocked())
                return EXIT_BLOCKED
            sources[name] = {"error": error.blocked(), "commands": [error.command]}
            warnings.append(
                f"{name} source unavailable ({error.reason}): {error.message}"
            )
            continue
        records[name] = fetched.pop("records")
        warnings += [
            f"{name}: {item.get('message')}" for item in fetched.pop("warnings")
        ]
        sources[name] = fetched

    groups = build_groups(records["runtime"], records["requests"])
    emit(
        {
            "status": "ok",
            "generatedAt": dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "query": {
                "scope": SCOPE,
                "project": PROJECT,
                "environment": args.environment,
                "since": args.since,
                "until": args.until,
                "deployment": args.deployment,
                "limit": args.limit,
                "runtimeQuery": args.runtime_query if "runtime" in selected else None,
                "requestsQuery": args.requests_query
                if "requests" in selected
                else None,
                "continued": bool(args.next),
            },
            "sources": sources,
            "warnings": warnings,
            "categoryCounts": {
                category: sum(
                    group["count"] for group in groups if group["category"] == category
                )
                for category in CATEGORIES
            },
            "groups": groups,
        }
    )
    return 0


def cmd_report_md(args: argparse.Namespace, _runner: Runner) -> int:
    try:
        payload = json.loads(Path(args.triage).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"Error: cannot read triage JSON: {error}", file=sys.stderr)
        return 2
    if not isinstance(payload, dict) or payload.get("status") != "ok":
        print(
            "Error: triage JSON is not an ok result; report the blocker instead.",
            file=sys.stderr,
        )
        return 2
    print(render_report(payload), end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vercel_triage", description="Read-only merkle-api Vercel log triage."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check = subparsers.add_parser("check", help="probe Vercel auth and project access")
    check.add_argument("--environment", default=ENVIRONMENT_DEFAULT)
    check.set_defaults(func=cmd_check)

    triage = subparsers.add_parser(
        "triage", help="fetch, group, and classify logs as JSON"
    )
    triage.add_argument("--since", default=SINCE_DEFAULT)
    triage.add_argument("--until")
    triage.add_argument("--deployment", help="deployment ID or URL")
    triage.add_argument("--environment", default=ENVIRONMENT_DEFAULT)
    triage.add_argument(
        "--limit",
        type=positive_limit,
        default=LIMIT_DEFAULT,
        help="record cap per source",
    )
    triage.add_argument("--source", choices=["all", *SOURCES], default="all")
    triage.add_argument(
        "--next",
        help="pagination.next cursor from a previous triage run's sources.<name>.next",
    )
    triage.add_argument("--runtime-query", default=RUNTIME_QUERY_DEFAULT)
    triage.add_argument("--requests-query", default=REQUESTS_QUERY_DEFAULT)
    triage.set_defaults(func=cmd_triage)

    report = subparsers.add_parser("report-md", help="render Markdown from triage JSON")
    report.add_argument("--triage", required=True, help="path to triage JSON")
    report.set_defaults(func=cmd_report_md)
    return parser


def main(argv: list[str] | None = None, runner: Runner = default_runner) -> int:
    args = build_parser().parse_args(argv)
    return int(args.func(args, runner))
