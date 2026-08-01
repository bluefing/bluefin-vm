[default]
_default:
    just --list {{ module_path() }} --unsorted

set shell := ["bash", "-euc"]
set script-interpreter := ["bash", "-euo", "pipefail"]

# rustup installs cargo under ~/.cargo/bin, which just's non-interactive recipe
# shells and the pre-commit hook don't inherit from the login profile. Put it
# on PATH so `cargo` is always found in any module that drives the crate.
export PATH := env('HOME') / '.cargo/bin' + ':' + env('PATH')

[private, script, no-exit-message]
_help file="help.md" width="90":
    # Render a module help file with glow, bat, or cat (first available).
    # Width controls glow's paragraph reflow. bat and cat don't reflow markdown.
    help="{{ module_directory() }}/{{ file }}"
    [[ ! -f "$help" ]] && { echo "No help available."; exit 0; }
    if command -v glow >/dev/null; then
        glow -w {{ width }} -p "$help"
    elif command -v bat >/dev/null; then
        bat --style=plain "$help"
    else
        cat "$help"
    fi

# Show module help — renders help.md from the calling module's directory.
[group('module')]
[script, no-exit-message]
[arg('debug', short = 'd', long, value = 'true')]
[arg('file', short = 'f', long)]
[arg('width', short = 'w', long)]
help $debug="" $file="help.md" $width="90" *submodules:
    subs={{ replace_regex(submodules, "[ \t]+", "::") }}
    path="{{ module_path() }}::${subs}"
    path="${path%::}::_help"
    path="${path#::}";

    [[ -n $debug ]] && {
        echo "debug::help module='{{ module_path() }}'" \
             "submodules='{{ submodules }}' subs='${subs}'" \
             "path='${path}'"
    } >&2

    # Explicit --justfile so the recursive call resolves the module::_help path
    # against this same justfile, independent of the caller's working directory.
    just --justfile "{{ justfile() }}" "$path" "$file" "$width"
