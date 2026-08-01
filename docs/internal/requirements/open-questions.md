# Open questions and decisions

Architectural decisions worth recording so they aren't re-litigated, and the
questions still genuinely open. The credential model itself lives in
`../design/access.md`; this file is the decision log around it.

## Decided

### No extra declarative framework (Terraform / Ansible / NixOS)

**Decision:** don't adopt Terraform, Ansible, or NixOS. The declarative story is
already covered by tools that fit the Bluefin/OCI/Tart shape, and each framework
would either duplicate a layer or fight the premise.

Three declarative layers already exist, each in the idiomatic tool:

| Layer | Role | Tool in use | Framework it displaces |
| --- | --- | --- | --- |
| System image | reproducible OS build | bootc `Containerfile` + `config.toml` | NixOS — this *is* declarative IaC, done the OCI way |
| VM lifecycle | cpu/mem/display spec → Tart | Rust CLI + `just` + per-VM profile | Terraform |
| User environment | dotfiles convergence | chezmoi (the customisation hook) | Ansible / home-manager |

- **NixOS** is a non-starter as the OS — it would mean discarding Bluefin, which
  is the whole point. The "declarative OS in a repo" role is filled by the
  `Containerfile`.
- **Terraform** wraps a state file around one local resource, and the disk-build
  pipeline is procedural, not resource-shaped. It would only earn its keep for a
  fleet or cloud-hosted Tart.
- **Ansible** is the closest fit but overlaps `provision.sh` (first-boot
  bootstrap) on one side and chezmoi (user convergence) on the other, for the sake
  of a ~50-line bootstrap seam. It pays off for day-2 convergence across many
  VMs, not first boot of one.

The only imperative code is the thin first-boot glue bridging host → guest for the
account; bash is the right size for it. Where we lean into "declarative" instead:
treat the per-VM profile (`config.toml`) and provisioning inputs as the explicit,
editable, re-appliable spec, with the CLI as the apply engine.

**Revisit if:** the project grows to manage many VMs, or moves to cloud-hosted
Tart (Cirrus) — then a Tart Terraform provider or Ansible day-2 layer becomes
worth the weight.

## Open

### sudo posture

Login password and sudo are independent choices (see `../design/access.md`).
Proposed **(a): password sudo** — a deliberate prompt guards a fat-fingered or
pasted root command, and it deletes the passwordless-sudo drop-in. Alternative
**(b):** follow Hashimoto's setup (passwordless sudo). Not yet confirmed.

### User customisation hook mechanism

After the account exists, let the user layer their own environment (the reference
model is a devcontainer dotfiles feature: `chezmoi init --apply <repo>` with the
host ssh-agent forwarded, no secrets on disk). Open:

- How the guest reaches a *private* dotfiles repo at first boot — agent forwarding,
  a scoped token, or deferring to the first interactive login.
- Whether the hook is chezmoi-specific or a generic "run this once, as the user"
  escape hatch that chezmoi is merely the common case of.
