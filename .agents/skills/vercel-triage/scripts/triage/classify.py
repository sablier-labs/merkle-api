"""Signature and category for one runtime log entry or CDN request record."""

from __future__ import annotations

import dataclasses
import json
from typing import Any

from triage.constants import (
    FUNCTION_TIMEOUT_PATTERN,
    INIT_TIMEOUT_PATTERN,
    IPFS_INPUT_KINDS,
    IPFS_UPSTREAM_KINDS,
    OOM_PATTERN,
    PANIC_PATTERN,
    PAYLOAD_TOO_LARGE_PATTERN,
    PLATFORM_INTERNAL_PATTERN,
    PROCESS_EXIT_PATTERN,
    SECRETS_ERROR_PATTERN,
)
from triage.redact import normalize

UNKNOWN_REASON = "No known signature matched; treat as a first-party defect until the evidence says otherwise."


@dataclasses.dataclass(frozen=True)
class Classification:
    signature: str
    category: str
    reason: str
    kind: str
    event: str = ""
    endpoint: str = "-"


def endpoint_from_path(path: Any) -> str:
    """`/api/eligibility?cid=...` -> `eligibility`. Drops the query string, which can carry addresses."""
    if not isinstance(path, str) or not path:
        return "-"
    bare = path.split("?", 1)[0].rstrip("/")
    if bare.startswith("/api/"):
        return bare.removeprefix("/api/") or "-"
    return bare or "/"


def parse_structured(message: Any) -> dict[str, Any] | None:
    """Return the first `logging::log_event` JSON object (one with an `event` key) in a message."""
    if not isinstance(message, str):
        return None
    decoder = json.JSONDecoder()
    for line in message.splitlines():
        start = line.find("{")
        if start < 0:
            continue
        try:
            payload, _ = decoder.raw_decode(line[start:])
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict) and isinstance(payload.get("event"), str):
            return payload
    return None


def _structured(payload: dict[str, Any], fallback_endpoint: str) -> Classification:
    event = payload["event"]
    endpoint = str(payload.get("endpoint") or fallback_endpoint)
    kind = str(payload.get("error_kind") or "")
    provider_status = payload.get("provider_status")
    detail = kind
    if kind and isinstance(provider_status, int):
        detail = f"{kind}/{provider_status}"
    signature = f"{event}:{detail}@{endpoint}" if detail else f"{event}@{endpoint}"

    def result(category: str, reason: str) -> Classification:
        return Classification(
            signature, category, reason, kind or event, event=event, endpoint=endpoint
        )

    if event == "eligibility_ipfs_error":
        if kind in IPFS_UPSTREAM_KINDS:
            return result(
                "upstream",
                "Pinata/IPFS gateway request failed or timed out; the handler returns 502/504 by design, which "
                "Railway retries once.",
            )
        if kind in IPFS_INPUT_KINDS:
            return result(
                "actionable",
                f"`{kind}` returns 500 by design (never a sub-500 verdict). Check whether callers send a bad or "
                "unpinned CID, or the pinned content is not a campaign, before changing code.",
            )
    elif event == "eligibility_malformed_tree":
        return result(
            "actionable",
            "Pinned campaign has an unparseable merkle_tree; check the create path for the CID.",
        )
    elif event == "eligibility_slow":
        return result(
            "expected",
            "Successful eligibility response above the 2 s slow threshold; latency signal only.",
        )
    elif event == "pinata_upload_error":
        return result(
            "upstream",
            "Pinata pinJSONToIPFS request failed; `create` returns 500 by design.",
        )
    elif event == "pinata_upload_parse_error":
        return result(
            "actionable",
            "Pinata upload response lacked `IpfsHash`; inspect `error_message` for an API shape change.",
        )
    return result(
        "actionable",
        f"Unrecognized structured event `{event}`/`{kind}`. {UNKNOWN_REASON}",
    )


def classify_runtime(record: dict[str, Any]) -> Classification:
    endpoint = endpoint_from_path(record.get("requestPath"))
    message = str(record.get("message") or "")

    payload = parse_structured(message)
    if payload is not None:
        return _structured(payload, endpoint)

    def result(kind: str, category: str, reason: str, body: str) -> Classification:
        return Classification(
            f"{kind}:{body}@{endpoint}", category, reason, kind, endpoint=endpoint
        )

    if match := SECRETS_ERROR_PATTERN.search(message):
        variant, key = match.group(1), match.group(2) or ""
        body = f"{variant}({key})" if key else variant
        return result(
            "secrets_loader",
            "actionable",
            "`utils::secrets::load` stopped startup; check `.env.production` and DOTENV_PRIVATE_KEY_PRODUCTION.",
            body,
        )
    if match := PANIC_PATTERN.search(message):
        location, rest = match.group(1), match.group(2)
        return result(
            "panic",
            "actionable",
            "Rust panic in first-party code.",
            f"{location} {normalize(rest.splitlines()[0] if rest else '')}".strip(),
        )
    if FUNCTION_TIMEOUT_PATTERN.search(message):
        return result(
            "function_timeout",
            "platform",
            "Vercel invocation timeout. If it clusters on eligibility/validity, compare the function maxDuration with "
            "the 12 s IPFS gateway timeout.",
            normalize(message),
        )
    if OOM_PATTERN.search(message):
        return result(
            "out_of_memory",
            "platform",
            "Lambda killed for memory; check payload size for the CID.",
            normalize(message),
        )
    if INIT_TIMEOUT_PATTERN.search(message):
        return result(
            "init_timeout",
            "platform",
            "Cold start exceeded the platform init limit.",
            normalize(message),
        )
    if PLATFORM_INTERNAL_PATTERN.search(message):
        return result(
            "platform_internal",
            "platform",
            "Vercel-internal function error code.",
            normalize(message),
        )
    if PAYLOAD_TOO_LARGE_PATTERN.search(message):
        return result(
            "payload_too_large",
            "actionable",
            "Request or response exceeded the function payload limit; check the endpoint's input or output size.",
            normalize(message),
        )
    if PROCESS_EXIT_PATTERN.search(message):
        return result(
            "process_exit",
            "actionable",
            "Lambda process exited or crashed; correlate with panic/secrets_loader groups on the same deployment.",
            normalize(message),
        )
    return result(
        "message", "actionable", UNKNOWN_REASON, normalize(message) or "<empty>"
    )


def classify_request(record: dict[str, Any]) -> Classification:
    """Classify a CDN request record that no fetched runtime entry explains."""
    endpoint = endpoint_from_path(record.get("requestPath"))
    status = record.get("httpStatus")
    status_text = str(status) if isinstance(status, int) else "unknown"
    signature = f"http_{status_text}@{endpoint}"

    def result(category: str, reason: str) -> Classification:
        return Classification(
            signature, category, reason, f"http_{status_text}", endpoint=endpoint
        )

    if status in (502, 504) and endpoint == "validity":
        return result(
            "upstream",
            "validity returns 502/504 for IPFS request/provider failures without logging them.",
        )
    if isinstance(status, int) and status >= 500:
        return result(
            "actionable",
            f"{status} with no matching runtime entry in the fetched window. eligibility logs every 5xx path and "
            "create logs Pinata failures (not CSV-parse 500s); inspect the request's runtime logs. The runtime "
            "fetch may also have hit --limit.",
        )
    if status == 401:
        return result(
            "expected", "Bearer rejection; `is_authorized` is fail-closed by design."
        )
    if status == 400 and endpoint == "eligibility":
        return result(
            "expected", "Not-eligible verdict (400) consumed by Railway and the portal."
        )
    if status == 400 and endpoint == "validity":
        return result("expected", "Bad CID or invalid campaign file (400) by design.")
    if isinstance(status, int) and 400 <= status < 500:
        return result(
            "expected", f"Client error {status}; no first-party failure signal."
        )
    return result("actionable", UNKNOWN_REASON)
