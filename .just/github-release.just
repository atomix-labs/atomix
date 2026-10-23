# Renders the next release's notes from .github/release-notes.md and the changes, as a release will.
check-github-release:
    #!/usr/bin/env bash
    set -euo pipefail
    git rev-parse -q --verify HEAD > /dev/null || exit 0
    mise exec -- python3 -B .just/github-release/notes.py --unreleased atomix-labs/atomix
