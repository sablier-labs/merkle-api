"""Group classified records by signature and attach CDN 5xx records to the runtime groups that explain them."""

from __future__ import annotations

import datetime as dt
from typing import Any

from triage.classify import (
    Classification,
    classify_request,
    classify_runtime,
    parse_structured,
)
from triage.constants import CATEGORIES
from triage.redact import sample

SAMPLE_REQUEST_IDS = 5
SAMPLE_DEPLOYMENTS = 5


def to_epoch_ms(value: Any) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return int(value if value > 1e11 else value * 1000)
    if isinstance(value, str) and value:
        try:
            parsed = dt.datetime.fromisoformat(value)
        except ValueError:
            return None
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt.UTC)
        return int(parsed.timestamp() * 1000)
    return None


def to_iso(epoch_ms: int | None) -> str:
    if epoch_ms is None:
        return ""
    return dt.datetime.fromtimestamp(epoch_ms / 1000, tz=dt.UTC).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )


class _Group:
    def __init__(self, classification: Classification, source: str) -> None:
        self.classification = classification
        self.source = source
        self.count = 0
        self.first_ms: int | None = None
        self.last_ms: int | None = None
        self.endpoints: set[str] = set()
        self.deployments: list[str] = []
        self.request_ids: list[str] = []
        self.levels: dict[str, int] = {}
        self.http_statuses: dict[str, int] = {}
        self.max_elapsed_ms: int | None = None
        self.sample_message = ""

    def add(self, record: dict[str, Any], *, endpoint: str) -> None:
        self.count += 1
        self.endpoints.add(endpoint)
        when = to_epoch_ms(record.get("timestamp"))
        if when is not None:
            self.first_ms = when if self.first_ms is None else min(self.first_ms, when)
            self.last_ms = when if self.last_ms is None else max(self.last_ms, when)
        deployment = record.get("deploymentId")
        if (
            isinstance(deployment, str)
            and deployment
            and deployment not in self.deployments
        ):
            self.deployments.append(deployment)
        self.add_request_id(record.get("requestId"))

    def add_request_id(self, request_id: Any) -> None:
        if (
            isinstance(request_id, str)
            and request_id
            and request_id not in self.request_ids
        ):
            self.request_ids.append(request_id)

    def add_status(self, status: Any) -> None:
        key = str(status) if isinstance(status, int) else "unknown"
        self.http_statuses[key] = self.http_statuses.get(key, 0) + 1

    def as_dict(self) -> dict[str, Any]:
        c = self.classification
        return {
            "signature": c.signature,
            "category": c.category,
            "reason": c.reason,
            "source": self.source,
            "event": c.event,
            "kind": c.kind,
            "endpoints": sorted(self.endpoints),
            "count": self.count,
            "firstSeen": to_iso(self.first_ms),
            "lastSeen": to_iso(self.last_ms),
            "levels": dict(sorted(self.levels.items())),
            "httpStatuses": dict(sorted(self.http_statuses.items())),
            "deploymentIds": self.deployments[:SAMPLE_DEPLOYMENTS],
            "deploymentCount": len(self.deployments),
            "sampleRequestIds": self.request_ids[:SAMPLE_REQUEST_IDS],
            "maxElapsedMs": self.max_elapsed_ms,
            "sampleMessage": self.sample_message,
        }


def build_groups(
    runtime: list[dict[str, Any]], requests: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    groups: dict[tuple[str, str], _Group] = {}
    group_by_request: dict[str, _Group] = {}

    for record in runtime:
        classification = classify_runtime(record)
        group = groups.setdefault(
            ("runtime", classification.signature), _Group(classification, "runtime")
        )
        group.add(record, endpoint=classification.endpoint)
        level = record.get("level")
        if isinstance(level, str) and level:
            group.levels[level] = group.levels.get(level, 0) + 1
        payload = parse_structured(record.get("message"))
        elapsed = payload.get("elapsed_ms") if payload else None
        if isinstance(elapsed, int):
            group.max_elapsed_ms = max(group.max_elapsed_ms or 0, elapsed)
        if not group.sample_message:
            group.sample_message = sample(str(record.get("message") or ""))
        request_id = record.get("requestId")
        if isinstance(request_id, str) and request_id:
            group_by_request.setdefault(request_id, group)

    for record in requests:
        request_id = record.get("requestId")
        explained = (
            group_by_request.get(request_id) if isinstance(request_id, str) else None
        )
        if explained is not None:
            explained.add_status(record.get("httpStatus"))
            continue
        classification = classify_request(record)
        group = groups.setdefault(
            ("requests", classification.signature), _Group(classification, "requests")
        )
        group.add(record, endpoint=classification.endpoint)
        group.add_status(record.get("httpStatus"))
        if not group.sample_message:
            method = record.get("requestMethod") or "-"
            group.sample_message = sample(
                f"{method} {record.get('httpStatus')} {classification.endpoint}"
            )

    order = {category: index for index, category in enumerate(CATEGORIES)}
    result = [group.as_dict() for group in groups.values()]
    result.sort(
        key=lambda item: (
            order.get(item["category"], 0),
            -item["count"],
            item["signature"],
        )
    )
    return result
