# docs — build and serve the documentation site (Zensical).
#
# uv comes from mise (pinned in mise.toml) and owns docs/.venv and the
# interpreter, so the site builds from a bare checkout after `mise install`.

import '../_common.just'

_docs_dir := justfile_directory() / "docs"

# Sync the venv from docs/pyproject.toml
[group('docs')]
sync:
    cd "{{ _docs_dir }}" && uv sync

# Serve the docs locally with live reload (default http://localhost:8000)
[group('docs')]
serve port="8000":
    cd "{{ _docs_dir }}" && uv run zensical serve -a "localhost:{{ port }}"

# Build the static site to docs/site/
[group('docs')]
build:
    cd "{{ _docs_dir }}" && uv run zensical build

# Remove the built site and the docs venv
[group('docs')]
[script]
clean:
    rm -rf "{{ _docs_dir }}/site" "{{ _docs_dir }}/.venv"
    echo "OK: docs cleaned."
