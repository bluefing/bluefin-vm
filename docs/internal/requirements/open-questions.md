# Open questions and decisions

Architectural decisions worth recording so they aren't re-litigated, and the questions still genuinely open. The
credential model itself lives in `../design/access.md`; this file is the decision log around it.

## Decided

### No extra declarative framework (Terraform / Ansible / NixOS)

**Decision:** *bluefin-vm's own machinery* doesn't adopt Terraform, Ansible, or NixOS. The two jobs the project itself
owns are already covered by tools that fit the Bluefin/OCI/Tart shape. Read each row as "for this job we use X, so the
project doesn't need Y":

| The project's job                             | What we use for it                         | So the project doesn't need |
| --------------------------------------------- | ------------------------------------------ | --------------------------- |
| Build the OS image, reproducibly              | bootc `Containerfile` + `config.toml`      | NixOS                       |
| Create and size the VM (cpu, memory, display) | Rust CLI + `just` + a saved per-VM profile | Terraform                   |

- **NixOS** is a non-starter as the OS — it would mean discarding Bluefin, which is the whole point. The "declarative OS
    in a repo" role is filled by the `Containerfile`.
- **Terraform** wraps a state file around one local resource, and the disk-build pipeline is procedural, not
    resource-shaped. It would only earn its keep for a fleet or cloud-hosted Tart.
- **Ansible** we don't adopt for the project's own machinery either: its natural home would be the first-boot account
    bootstrap, but that's a ~50-line seam where bash is right-sized, and day-2 convergence across many VMs (where
    Ansible earns its keep) isn't the target.

Setting up the **user's environment** is deliberately *not* the project's choice to make. bluefin-vm provides a hook
(below) that runs whatever the user brings — chezmoi, `ansible-pull`, home-manager, or a plain script. So none of those
is "displaced"; the project simply doesn't bundle one. chezmoi is the maintainer's own tool, an example, not a mandate.

The only imperative code is the thin first-boot glue bridging host → guest for the account; bash is the right size for
it. Where we lean into "declarative" instead: treat the per-VM profile (`config.toml`) and provisioning inputs as the
explicit, editable, re-appliable spec, with the CLI as the apply engine.

**Revisit if:** the project grows to manage many VMs, or moves to cloud-hosted Tart (Cirrus) — then a Tart Terraform
provider or Ansible day-2 layer becomes worth the weight.

### sudo posture — a TUI toggle

**Decision:** passwordless `sudo` is its own toggle in `bluefin-vm tui`, independent of everything else. Login password
and sudo were always independent choices; today sudo is instead a side effect of the (now-dropped) autologin flag, so
this decouples it.

- **Default: `sudo` prompts** for the login password. Not for security (the account is admin either way) but as a
    speed-bump, so a mistyped or pasted command can't silently run as root. Prompting is the default state, so this
    means simply *not* writing the `NOPASSWD` sudoers file.
- **Toggle on: passwordless** — write the `NOPASSWD` sudoers drop-in, matching Hashimoto. For the user who'd rather
    never be asked.

Implementation (delivered on the drop-autologin branch): a `sudo_password` config field (`true` = prompts, the default)
\+ a TUI toggle, a `passwordless-sudo` flag the host writes when it's off, and a branch in `provision.sh` that writes the
sudoers rule when that flag is present.

## Open

### User customisation hook mechanism

After the account exists, let the user layer their own environment (the reference model is a devcontainer dotfiles
feature: `chezmoi init --apply <repo>` with the host ssh-agent forwarded, no secrets on disk). Open:

- How the guest reaches a *private* dotfiles repo at first boot — agent forwarding, a scoped token, or deferring to the
    first interactive login.
- Whether the hook is chezmoi-specific or a generic "run this once, as the user" escape hatch that chezmoi is merely the
    common case of.
