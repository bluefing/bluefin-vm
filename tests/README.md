# Tests

Tests are organised by **tier** — the layered strategy is in
[`../docs/internal/design/testing-strategy.md`](../docs/internal/design/testing-strategy.md).
The rule that keeps the separation from eroding:

> **A test goes in the directory for the cheapest tier that can run it. Never add
> a docker- or VM-dependent test to `offline/`.**

| Dir | Tier | What | Run with | Needs |
| --- | --- | --- | --- | --- |
| `offline/` | 0 | bats contracts: script arg/dry-run, recipe wiring, `config.toml` | `just test` | nothing |
| `integration/` | 1 | `provision.sh` in a container, across the config matrix | `just test-integration` | Docker |
| `e2e/` | 2 | `guest-checks.sh` in a booted VM, over ssh | `just test-e2e` (VM up first) | a running VM |

The **unit** tests (Tier 0) are the Rust `#[cfg(test)]` modules in `cli/src/**`,
run by `cargo test` (via `just test`). They live in the crate by Rust
convention — they reach private items — so there is no `tests/unit/`.

`integration/assert-posture.sh` is deliberately a standalone script, not bats:
it runs *inside* the target (the Tier 1 container, and later the Tier 2 VM over
ssh), which is where the account state it checks actually exists. Reusing one
assertion set across both tiers keeps them from drifting apart.
