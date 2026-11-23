from __future__ import annotations

import io
import json
import subprocess
import tempfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from typing import Any
from unittest import TestCase, main

from triage.classify import classify_request, classify_runtime, endpoint_from_path
from triage.cli import main as cli_main
from triage.groups import build_groups
from triage.redact import normalize, redact, sample
from triage.report import render_report
from triage.vercel import VercelError, fetch_source, query_argv, run_page

REQ_A = "iad1::fra1::abcd5-1763900000000-0123456789ab"
REQ_B = "iad1::fra1::efgh6-1763900000001-ba9876543210"
DPL = "dpl_7gTzQk3n8YhP2wX"
CID = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"
TS = 1763900000000

LOGIN_REQUIRED = {
    "status": "error",
    "reason": "login_required",
    "message": "You're not logged in.",
    "hint": "Run `vercel login`. Or set VERCEL_TOKEN or pass --token, then retry.",
    "retryable": False,
    "userActionRequired": True,
    "next": [{"command": "vercel login --scope sablier", "when": "To sign in"}],
}


def event_line(event: str, **fields: Any) -> str:
    return json.dumps(
        {
            "endpoint": "eligibility",
            "cid": CID,
            **fields,
            "level": "error",
            "event": event,
        }
    )


def log(
    message: str,
    *,
    request_id: str = REQ_A,
    level: str = "error",
    path: str = "/api/eligibility",
    ts: int = TS,
):
    return {
        "type": "log",
        "data": {
            "timestamp": ts,
            "level": level,
            "message": message,
            "requestId": request_id,
            "deploymentId": DPL,
            "requestPath": f"{path}?cid={CID}&address=0xabc" if path else None,
            "environment": "production",
            "stream": "stderr",
        },
    }


def request(
    status: int, *, request_id: str = REQ_B, path: str = "/api/eligibility"
) -> dict[str, Any]:
    return {
        "type": "request",
        "data": {
            "timestamp": TS,
            "httpStatus": status,
            "requestId": request_id,
            "deploymentId": DPL,
            "requestPath": path,
            "requestMethod": "GET",
        },
    }


def summary(
    *, has_more: bool = False, next_cursor: str | None = None, returned: int = 0
) -> dict[str, Any]:
    return {
        "type": "summary",
        "recordsReturned": returned,
        "pagination": {"hasMore": has_more, "next": next_cursor},
    }


def jsonl(*objects: dict[str, Any]) -> str:
    return "".join(json.dumps(item) + "\n" for item in objects)


class FakeRunner:
    """Replays queued (stdout, stderr, returncode) results and records every argv."""

    def __init__(self, *results: tuple[str, str, int]) -> None:
        self.results = list(results)
        self.calls: list[list[str]] = []

    def __call__(self, argv: list[str]) -> subprocess.CompletedProcess[str]:
        self.calls.append(argv)
        stdout, stderr, code = self.results.pop(0)
        return subprocess.CompletedProcess(argv, code, stdout, stderr)


def runtime_records(*items: dict[str, Any]) -> list[dict[str, Any]]:
    return [item["data"] for item in items]


class NormalizationTest(TestCase):
    def test_strips_high_cardinality_values(self) -> None:
        text = (
            f"timeout for {CID} addr 0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491 req {REQ_A} "
            "id 123e4567-e89b-12d3-a456-426614174000 after 12.5s and 300ms hash deadbeefcafebabe00 count 42"
        )
        self.assertEqual(
            normalize(text),
            "timeout for <cid> addr <hex> req <request-id> id <uuid> after <duration> and <duration> hash <hex> "
            "count <n>",
        )

    def test_redacts_credentials(self) -> None:
        text = (
            'Authorization: Bearer abc.def-123 "token":"s3cr3t" https://gw/x?pinataGatewayToken=zzz&a=1 '
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl PINATA_ACCESS_TOKEN must be set"
        )
        redacted = redact(text)
        for secret in ("abc.def-123", "s3cr3t", "zzz", "eyJhbGci"):
            self.assertNotIn(secret, redacted)
        self.assertIn("PINATA_ACCESS_TOKEN must be set", redacted)
        self.assertIn("a=1", redacted)

    def test_sample_truncates_and_collapses(self) -> None:
        self.assertEqual(sample("a\n  b"), "a b")
        self.assertTrue(sample("x" * 1000).endswith("…"))
        self.assertLessEqual(len(sample("x" * 1000)), 300)

    def test_endpoint_drops_query_string(self) -> None:
        self.assertEqual(
            endpoint_from_path("/api/eligibility?cid=x&address=0xabc"), "eligibility"
        )
        self.assertEqual(endpoint_from_path(None), "-")
        self.assertEqual(endpoint_from_path("/favicon.ico"), "/favicon.ico")


class ClassifierTest(TestCase):
    def runtime(self, message: str, **kwargs: Any):
        return classify_runtime(log(message, **kwargs)["data"])

    def test_ipfs_provider_failures_are_upstream(self) -> None:
        for kind in (
            "request_timeout",
            "request_connect",
            "request_decode",
            "request_error",
        ):
            result = self.runtime(
                event_line(
                    "eligibility_ipfs_error", error_kind=kind, response_status=504
                )
            )
            self.assertEqual(result.category, "upstream", kind)
            self.assertEqual(
                result.signature, f"eligibility_ipfs_error:{kind}@eligibility"
            )
        result = self.runtime(
            event_line(
                "eligibility_ipfs_error", error_kind="upstream", provider_status=429
            )
        )
        self.assertEqual(
            (result.category, result.signature),
            ("upstream", "eligibility_ipfs_error:upstream/429@eligibility"),
        )

    def test_ipfs_input_kinds_stay_actionable(self) -> None:
        for kind in ("invalid_cid", "not_found", "deserialize"):
            result = self.runtime(
                event_line(
                    "eligibility_ipfs_error", error_kind=kind, response_status=500
                )
            )
            self.assertEqual(result.category, "actionable", kind)
            self.assertIn("sub-500", result.reason)

    def test_structured_first_party_and_expected_events(self) -> None:
        self.assertEqual(
            self.runtime(event_line("eligibility_malformed_tree")).category,
            "actionable",
        )
        slow = self.runtime(
            event_line("eligibility_slow", elapsed_ms=2400), level="info"
        )
        self.assertEqual(
            (slow.category, slow.signature),
            ("expected", "eligibility_slow@eligibility"),
        )
        upload = self.runtime(
            json.dumps(
                {
                    "endpoint": "create",
                    "error_kind": "request_timeout",
                    "event": "pinata_upload_error",
                }
            ),
            path="/api/create",
        )
        self.assertEqual((upload.category, upload.endpoint), ("upstream", "create"))
        parse = self.runtime(
            json.dumps({"endpoint": "create", "event": "pinata_upload_parse_error"})
        )
        self.assertEqual(parse.category, "actionable")
        unknown = self.runtime(json.dumps({"endpoint": "health", "event": "new_event"}))
        self.assertEqual(unknown.category, "actionable")

    def test_panics_and_secrets_loader_are_actionable(self) -> None:
        panic = self.runtime(
            "thread 'main' panicked at src/services/ipfs.rs:120:66:\nPINATA_API_KEY must be set: NotPresent"
        )
        self.assertEqual(panic.category, "actionable")
        self.assertEqual(panic.kind, "panic")
        self.assertIn(
            "src/services/ipfs.rs PINATA_API_KEY must be set", panic.signature
        )

        secrets = self.runtime('Error: MissingKey("IPFS_GATEWAY")', path="")
        self.assertEqual(
            (secrets.category, secrets.signature),
            ("actionable", "secrets_loader:MissingKey(IPFS_GATEWAY)@-"),
        )
        self.assertEqual(
            self.runtime("Error: MissingPrivateKey").signature,
            "secrets_loader:MissingPrivateKey@eligibility",
        )

    def test_platform_signals(self) -> None:
        cases = {
            "Task timed out after 15.00 seconds": "function_timeout",
            "Vercel Runtime Timeout Error: Task timed out after 300 seconds": "function_timeout",
            "Runtime exited with error: signal: killed": "out_of_memory",
            "INIT_REPORT Init Duration: 10000.12 ms Phase: init Status: timeout": "init_timeout",
            "INTERNAL_FUNCTION_INVOCATION_ERROR": "platform_internal",
        }
        for message, kind in cases.items():
            result = self.runtime(message)
            self.assertEqual(
                (result.category, result.kind), ("platform", kind), message
            )

    def test_crashes_and_unknown_messages_are_actionable(self) -> None:
        self.assertEqual(
            self.runtime("Runtime exited with error: exit status 101").kind,
            "process_exit",
        )
        self.assertEqual(
            self.runtime("FUNCTION_RESPONSE_PAYLOAD_TOO_LARGE").kind,
            "payload_too_large",
        )
        unknown = self.runtime(f"something odd for {CID}")
        self.assertEqual(
            (unknown.category, unknown.signature),
            ("actionable", "message:something odd for <cid>@eligibility"),
        )

    def test_unexplained_request_records(self) -> None:
        self.assertEqual(classify_request(request(502)["data"]).category, "actionable")
        self.assertEqual(classify_request(request(401)["data"]).category, "expected")
        validity = request(504, path="/api/validity")["data"]
        self.assertEqual(classify_request(validity).category, "upstream")
        self.assertEqual(
            classify_request(request(500, path="/api/validity")["data"]).category,
            "actionable",
        )
        not_eligible = classify_request(request(400)["data"])
        self.assertEqual(
            (not_eligible.category, not_eligible.signature),
            ("expected", "http_400@eligibility"),
        )
        self.assertEqual(
            classify_request(request(400, path="/api/validity")["data"]).category,
            "expected",
        )


class GroupingTest(TestCase):
    def test_groups_by_signature_and_tracks_metadata(self) -> None:
        timeout = event_line(
            "eligibility_ipfs_error", error_kind="request_timeout", elapsed_ms=12001
        )
        groups = build_groups(
            runtime_records(
                log(timeout, request_id=REQ_A, ts=TS),
                log(
                    timeout.replace(
                        CID, "QmZZZPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"
                    ),
                    request_id=REQ_B,
                    ts=TS + 5000,
                ),
                log(
                    "thread 'main' panicked at src/controller/create.rs:47:1:\nboom",
                    request_id="r3",
                ),
            ),
            [],
        )
        self.assertEqual(
            [group["category"] for group in groups], ["actionable", "upstream"]
        )
        upstream = groups[1]
        self.assertEqual(upstream["count"], 2)
        self.assertEqual(upstream["endpoints"], ["eligibility"])
        self.assertEqual(upstream["sampleRequestIds"], [REQ_A, REQ_B])
        self.assertEqual(upstream["deploymentIds"], [DPL])
        self.assertEqual(upstream["maxElapsedMs"], 12001)
        self.assertEqual(upstream["firstSeen"], "2025-11-23T12:13:20Z")
        self.assertEqual(upstream["lastSeen"], "2025-11-23T12:13:25Z")
        self.assertEqual(upstream["levels"], {"error": 2})

    def test_requests_attach_to_explaining_runtime_group(self) -> None:
        groups = build_groups(
            runtime_records(
                log(
                    event_line(
                        "eligibility_ipfs_error",
                        error_kind="upstream",
                        provider_status=503,
                    )
                )
            ),
            runtime_records(
                request(502, request_id=REQ_A), request(500, request_id=REQ_B)
            ),
        )
        by_signature = {group["signature"]: group for group in groups}
        self.assertEqual(
            by_signature["eligibility_ipfs_error:upstream/503@eligibility"][
                "httpStatuses"
            ],
            {"502": 1},
        )
        unexplained = by_signature["http_500@eligibility"]
        self.assertEqual(
            (unexplained["category"], unexplained["source"]), ("actionable", "requests")
        )
        self.assertEqual(unexplained["sampleRequestIds"], [REQ_B])

    def test_naive_iso_timestamp_is_utc(self) -> None:
        record = runtime_records(log("x"))[0]
        record["timestamp"] = "2025-11-23T12:13:20"
        self.assertEqual(
            build_groups([record], [])[0]["firstSeen"], "2025-11-23T12:13:20Z"
        )

    def test_sample_message_is_redacted(self) -> None:
        groups = build_groups(
            runtime_records(log("upstream said Authorization: Bearer leaked-token")), []
        )
        self.assertNotIn("leaked-token", groups[0]["sampleMessage"])


class TransportTest(TestCase):
    def test_query_argv_pins_scope_project_environment(self) -> None:
        argv = query_argv(
            "requests",
            environment="production",
            since="7d",
            until="1h",
            deployment=DPL,
            query="httpStatus >= 500",
            limit=5,
        )
        self.assertEqual(
            argv,
            [
                "vercel", "logs", "requests", "--scope", "sablier", "--project", "merkle-api",
                "--environment", "production", "--since", "7d", "--until", "1h", "--deployment", DPL,
                "--query", "httpStatus >= 500", "--json", "--limit", "5",
            ],
        )  # fmt: skip

    def test_cli_error_on_stderr_maps_to_blocked(self) -> None:
        runner = FakeRunner(("", json.dumps(LOGIN_REQUIRED) + "\n", 1))
        with self.assertRaises(VercelError) as raised:
            run_page(runner, ["vercel", "logs"])
        blocked = raised.exception.blocked()
        self.assertEqual(blocked["status"], "blocked")
        self.assertEqual(blocked["reason"], "login_required")
        self.assertIn("vercel login --scope sablier", blocked["fix"])
        self.assertIn("VERCEL_TOKEN", blocked["fix"])
        self.assertEqual(blocked["next"], ["vercel login --scope sablier"])

    def test_subprocess_timeout_names_retry_fix(self) -> None:
        def slow(argv: list[str]) -> subprocess.CompletedProcess[str]:
            raise subprocess.TimeoutExpired(argv, 300)

        with self.assertRaises(VercelError) as raised:
            run_page(slow, ["vercel", "logs"])
        self.assertIn("Retry once later", raised.exception.blocked()["fix"])

    def test_requests_continuation_keeps_subcommand(self) -> None:
        runner = FakeRunner(
            (jsonl(request(502), summary(has_more=True, next_cursor="R1")), "", 0),
            (jsonl(request(504), summary()), "", 0),
        )
        fetch_source(runner, "requests", first_argv=None, cursor="R0", limit=5)
        self.assertEqual(
            [call[:4] for call in runner.calls],
            [
                ["vercel", "logs", "requests", "--next"],
                ["vercel", "logs", "requests", "--next"],
            ],
        )

    def test_permission_denied_names_access_fix(self) -> None:
        payload = {
            **LOGIN_REQUIRED,
            "reason": "permission_denied",
            "message": "Forbidden",
        }
        with self.assertRaises(VercelError) as raised:
            run_page(FakeRunner((json.dumps(payload), "", 1)), ["vercel", "logs"])
        self.assertIn("sablier team", raised.exception.blocked()["fix"])

    def test_missing_cli_and_missing_summary(self) -> None:
        def missing(argv: list[str]) -> subprocess.CompletedProcess[str]:
            raise FileNotFoundError(argv[0])

        with self.assertRaises(VercelError) as raised:
            run_page(missing, ["vercel", "logs"])
        self.assertEqual(raised.exception.reason, "vercel_missing")
        with self.assertRaises(VercelError) as raised:
            run_page(FakeRunner(("garbage", "boom", 1)), ["vercel", "logs"])
        self.assertEqual(raised.exception.reason, "cli_failed")

    def test_pages_through_next_cursor_without_conflicting_flags(self) -> None:
        runner = FakeRunner(
            (
                jsonl(log("a"), log("b"), summary(has_more=True, next_cursor="CUR1")),
                "",
                0,
            ),
            (jsonl(log("c"), summary(has_more=True, next_cursor="CUR2")), "", 0),
        )

        def first(limit: int) -> list[str]:
            return query_argv(
                "runtime",
                environment="production",
                since="7d",
                until=None,
                deployment=None,
                query="level:error",
                limit=limit,
            )

        fetched = fetch_source(
            runner, "runtime", first_argv=first, cursor=None, limit=3
        )
        self.assertEqual(fetched["recordCount"], 3)
        self.assertTrue(fetched["hasMore"])
        self.assertEqual(fetched["next"], "CUR2")
        self.assertEqual(runner.calls[0][-2:], ["--limit", "3"])
        self.assertEqual(
            runner.calls[1],
            ["vercel", "logs", "runtime", "--next", "CUR1", "--json", "--limit", "1"],
        )
        self.assertEqual(len(fetched["commands"]), 2)

    def test_stops_when_summary_has_no_more(self) -> None:
        runner = FakeRunner(
            (jsonl(log("a"), summary(has_more=False, next_cursor=None)), "", 0)
        )
        fetched = fetch_source(
            runner, "runtime", first_argv=None, cursor="START", limit=10
        )
        self.assertEqual(
            runner.calls[0][:5], ["vercel", "logs", "runtime", "--next", "START"]
        )
        self.assertEqual(
            (fetched["recordCount"], fetched["hasMore"], fetched["next"]),
            (1, False, None),
        )


class CliTest(TestCase):
    def run_cli(
        self, argv: list[str], runner: FakeRunner
    ) -> tuple[int, dict[str, Any]]:
        out = io.StringIO()
        with redirect_stdout(out):
            code = cli_main(argv, runner=runner)
        return code, json.loads(out.getvalue())

    def test_check_blocked_and_ok(self) -> None:
        code, payload = self.run_cli(
            ["check"], FakeRunner(("", json.dumps(LOGIN_REQUIRED), 1))
        )
        self.assertEqual(
            (code, payload["status"], payload["reason"]),
            (3, "blocked", "login_required"),
        )
        code, payload = self.run_cli(["check"], FakeRunner((jsonl(summary()), "", 0)))
        self.assertEqual((code, payload["status"]), (0, "ok"))

    def test_triage_fetches_both_sources(self) -> None:
        runner = FakeRunner(
            (jsonl(log(event_line("eligibility_malformed_tree")), summary()), "", 0),
            (jsonl(request(502, request_id=REQ_A), request(504), summary()), "", 0),
        )
        code, payload = self.run_cli(["triage", "--since", "1d"], runner)
        self.assertEqual(code, 0)
        self.assertEqual([call[2] for call in runner.calls], ["runtime", "requests"])
        self.assertIn("httpStatus >= 500", runner.calls[1])
        self.assertEqual(payload["categoryCounts"]["actionable"], 2)
        self.assertEqual(payload["sources"]["requests"]["recordCount"], 2)

    def test_triage_blocks_on_login_and_tolerates_requests_failure(self) -> None:
        code, payload = self.run_cli(
            ["triage"], FakeRunner(("", json.dumps(LOGIN_REQUIRED), 1))
        )
        self.assertEqual((code, payload["status"]), (3, "blocked"))

        api_error = {
            "status": "error",
            "reason": "api_error",
            "message": "requests need Observability",
        }
        runner = FakeRunner(
            (jsonl(log("x"), summary()), "", 0), ("", json.dumps(api_error), 1)
        )
        code, payload = self.run_cli(["triage"], runner)
        self.assertEqual((code, payload["status"]), (0, "ok"))
        self.assertEqual(payload["sources"]["requests"]["error"]["reason"], "api_error")
        self.assertTrue(
            any(
                "requests source unavailable" in warning
                for warning in payload["warnings"]
            )
        )

    def test_next_requires_single_source(self) -> None:
        with redirect_stderr(io.StringIO()) as stderr:
            code = cli_main(["triage", "--next", "CUR"], runner=FakeRunner())
        self.assertEqual(code, 2)
        self.assertIn("--source runtime", stderr.getvalue())

    def test_next_continues_one_source(self) -> None:
        runner = FakeRunner((jsonl(log("x"), summary()), "", 0))
        code, payload = self.run_cli(
            ["triage", "--source", "runtime", "--next", "CUR"], runner
        )
        self.assertEqual(code, 0)
        self.assertEqual(
            runner.calls,
            [
                [
                    "vercel",
                    "logs",
                    "runtime",
                    "--next",
                    "CUR",
                    "--json",
                    "--limit",
                    "1000",
                ]
            ],
        )
        self.assertTrue(payload["query"]["continued"])


class ReportTest(TestCase):
    def test_renders_sections_tables_and_commands(self) -> None:
        runner = FakeRunner(
            (
                jsonl(
                    log(
                        event_line(
                            "eligibility_ipfs_error", error_kind="request_timeout"
                        )
                    ),
                    log(
                        "thread 'main' panicked at src/x.rs:1:2:\nbad | pipe",
                        request_id="r9",
                    ),
                    log(
                        event_line("eligibility_slow", elapsed_ms=2500),
                        request_id="r10",
                        level="info",
                    ),
                    summary(has_more=True, next_cursor="CUR"),
                ),
                "",
                0,
            ),
            (jsonl(summary()), "", 0),
        )
        out = io.StringIO()
        with redirect_stdout(out):
            cli_main(["triage", "--limit", "3"], runner=runner)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "triage.json"
            path.write_text(out.getvalue())
            rendered = io.StringIO()
            with redirect_stdout(rendered):
                self.assertEqual(
                    cli_main(["report-md", "--triage", str(path)], runner=FakeRunner()),
                    0,
                )
        report = rendered.getvalue()
        for heading in (
            "## Findings Summary",
            "### Actionable (1 groups)",
            "### Upstream (1 groups)",
            "### Platform (0 groups)",
            "### Expected (1 groups)",
            "## Suggested Fixes",
            "<details>",
        ):
            self.assertIn(heading, report)
        self.assertIn("bad \\| pipe", report)
        self.assertIn("more available", report)
        self.assertIn(
            "vercel logs runtime --scope sablier --project merkle-api", report
        )
        self.assertIn(f"--query 'requestId:\"{REQ_A}\"'", report)
        self.assertIn("Max elapsed 2500 ms", report)

    def test_empty_report_says_no_changes(self) -> None:
        report = render_report(
            {"status": "ok", "query": {}, "sources": {}, "groups": []}
        )
        self.assertIn("No actionable groups; no code changes are needed.", report)
        self.assertIn("| None | - |", report)

    def test_backtick_in_signature_keeps_code_span(self) -> None:
        group = {"signature": "message:bad `x`@-", "category": "actionable"}
        report = render_report({"query": {}, "sources": {}, "groups": [group]})
        self.assertIn("`message:bad 'x'@-`", report)
        self.assertNotIn("`x`", report)

    def test_report_md_rejects_blocked_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "blocked.json"
            path.write_text(json.dumps({"status": "blocked"}))
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(
                    cli_main(["report-md", "--triage", str(path)], runner=FakeRunner()),
                    2,
                )


if __name__ == "__main__":
    main()
