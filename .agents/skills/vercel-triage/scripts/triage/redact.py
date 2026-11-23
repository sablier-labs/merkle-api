"""Redaction and normalization of log text before it reaches stdout."""

from __future__ import annotations

import re

SAMPLE_MAX_CHARS = 300
SIGNATURE_MAX_CHARS = 160

_REDACTIONS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(
            r"(?i)(\"?authorization\"?\s*[:=]\s*)(\"[^\"]*\"|(?:bearer\s+)?[^\s,}]+)"
        ),
        r"\1[REDACTED]",
    ),
    (re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]+"), "Bearer [REDACTED]"),
    (
        re.compile(r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"),
        "[REDACTED_JWT]",
    ),
    (
        re.compile(
            r"(?i)([?&][A-Za-z_]*(?:token|key|secret|signature)[A-Za-z_]*=)[^&\s\"']+"
        ),
        r"\1[REDACTED]",
    ),
    (
        re.compile(
            r"(?i)(\"?[A-Za-z_]*(?:token|secret|api[_-]?key|password)[A-Za-z_]*\"?\s*[:=]\s*)(?!\[REDACTED)(\"[^\"]*\"|[^\s,}&]+)"
        ),
        r'\1"[REDACTED]"',
    ),
    (re.compile(r"\b(?:sk|pk|vercel|vc)_[A-Za-z0-9]{20,}\b"), "[REDACTED_KEY]"),
)

_NORMALIZATIONS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(
            r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b"
        ),
        "<uuid>",
    ),
    (
        re.compile(r"\b[a-z]{3}\d(?:::[a-z]{3}\d)*::[a-z0-9]+-\d+-[0-9a-f]+\b"),
        "<request-id>",
    ),
    (re.compile(r"\bQm[1-9A-HJ-NP-Za-km-z]{44}\b"), "<cid>"),
    (re.compile(r"\bb[a-z2-7]{50,}\b"), "<cid>"),
    (re.compile(r"\b0x[0-9a-fA-F]+\b"), "<hex>"),
    (
        re.compile(r"\b\d+(?:\.\d+)?\s?(?:ms|s|sec|secs|seconds|min|minutes)\b"),
        "<duration>",
    ),
    (re.compile(r"\b[0-9a-fA-F]{12,}\b"), "<hex>"),
    (re.compile(r"\b\d+(?:\.\d+)?\b"), "<n>"),
)


def redact(text: str) -> str:
    """Mask credentials: Authorization headers, bearer values, JWTs, and token-like fields or query parameters."""
    for pattern, replacement in _REDACTIONS:
        text = pattern.sub(replacement, text)
    return text


def sample(text: str) -> str:
    """Redacted single-line sample, truncated for tables."""
    collapsed = re.sub(r"\s+", " ", redact(text)).strip()
    if len(collapsed) > SAMPLE_MAX_CHARS:
        return collapsed[: SAMPLE_MAX_CHARS - 1] + "…"
    return collapsed


def normalize(text: str) -> str:
    """Strip high-cardinality values so equivalent messages share one signature."""
    text = redact(text)
    for pattern, replacement in _NORMALIZATIONS:
        text = pattern.sub(replacement, text)
    text = re.sub(r"\s+", " ", text).strip()
    return text[:SIGNATURE_MAX_CHARS]
