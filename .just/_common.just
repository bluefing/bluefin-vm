[default]
_default:
    just --list {{ module_path() }} --unsorted

set shell := ["bash", "-euc"]
set script-interpreter := ["bash", "-euo", "pipefail"]
