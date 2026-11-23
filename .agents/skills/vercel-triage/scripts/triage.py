#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///

from triage import main

if __name__ == "__main__":
    raise SystemExit(main())
