# docs — build and serve the documentation site (Zensical).
#
# uv isn't assumed on PATH: it's pip-installed into a project-local venv
# (docs/.venv, gitignored) that also serves as uv's project environment, so the
# whole toolchain is self-contained and reproducible from a bare checkout.

import '../_common.just'

_docs_dir := justfile_directory() / "docs"
_venv := _docs_dir / ".venv"
_uv := _venv / "bin/uv"
_uv_env := "UV_PROJECT_ENVIRONMENT=" + _venv
_uv_run := _uv_env + " " + _uv + " run --no-sync"

# Bootstrap uv into the docs venv (one-time; a no-op once present).
[private]
[script]
_ensure-uv:
    command -v python3 >/dev/null || { echo "docs: python3 is required" >&2; exit 1; }
    if [[ -x "{{ _uv }}" ]]; then exit 0; fi
    python3 -m venv "{{ _venv }}"
    "{{ _venv }}/bin/pip" install -q uv
    cd "{{ _docs_dir }}" && {{ _uv_env }} "{{ _uv }}" sync

# Sync the venv from docs/pyproject.toml
[group('docs')]
sync: _ensure-uv
    cd "{{ _docs_dir }}" && {{ _uv_env }} "{{ _uv }}" sync

# Serve the docs locally with live reload (default http://localhost:8000)
[group('docs')]
serve port="8000": _ensure-uv
    cd "{{ _docs_dir }}" && {{ _uv_run }} zensical serve -a "localhost:{{ port }}"

# Build the static site to docs/site/
[group('docs')]
build: _ensure-uv
    cd "{{ _docs_dir }}" && {{ _uv_run }} zensical build

# Remove the built site and the docs venv
[group('docs')]
[script]
clean:
    rm -rf "{{ _docs_dir }}/site" "{{ _venv }}"
    echo "OK: docs cleaned."
