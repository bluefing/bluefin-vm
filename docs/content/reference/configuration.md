# VM profiles

Settings live in `~/.config/bluefin-vm/config.toml` (`$XDG_CONFIG_HOME` is honoured if set). The file holds one profile
per VM name, and each profile has three optional groups. Nothing is required: a group, or the whole file, may be
missing. Paths may start with `~`.

```toml
[vms.work.account]
user = "alice"
ssh_key = "~/.ssh/id_ed25519.pub"
sudo_password = true
ssh_password_auth = false

[vms.work.share]
directory = "~/work-share"
read_only = true

[vms.work.resources]
cpu = 8
memory_mib = 8192
refit = false
display = "2560x1600"
scale = 150
```

## account

Who the VM creates at first boot.

| Key                 | Type                  | Meaning                                                      |
| ------------------- | --------------------- | ------------------------------------------------------------ |
| `user`              | string                | The account to create, in `wheel`.                           |
| `ssh_key`           | path, or `"disabled"` | The **public** key to authorise; `"disabled"` installs none. |
| `sudo_password`     | boolean               | Whether `sudo` asks for the login password. Default `true`.  |
| `ssh_password_auth` | boolean               | Whether sshd accepts password login. Default `true`.         |

## share

The host directory mounted into the guest as `bluefin-share`.

| Key         | Type    | Meaning                                                    |
| ----------- | ------- | ---------------------------------------------------------- |
| `directory` | path    | The host directory to mount.                               |
| `read_only` | boolean | Whether the guest is denied write access. Default `false`. |

## resources

The VM's hardware, plus the guest desktop scale.

| Key          | Type    | Meaning                                                              |
| ------------ | ------- | -------------------------------------------------------------------- |
| `cpu`        | integer | Virtual cores.                                                       |
| `memory_mib` | integer | Memory in MiB, so the TUI's "Memory (GiB)" of 8 is `8192`.           |
| `refit`      | boolean | Whether Tart resizes the guest to follow the window. Default `true`. |
| `display`    | string  | Guest resolution as `WIDTHxHEIGHT`. Needs `refit = false`.           |
| `scale`      | integer | Desktop scale as a percentage. Needs `refit = false`.                |

`display` and `scale` need refit off because with it on the guest follows the window, so no fixed resolution or scale
can hold.

## Precedence

A command-line flag wins for that run, then the profile, then the built-in default. Passing `--share` therefore mounts a
different directory once without editing the file, while `bluefin-vm tui` changes it for every later run.

## What writes the file

`bluefin-vm tui` writes the profile you edited when you save. So does `bluefin-vm up` when it provisions a fresh VM: it
records the account it resolved, which is your macOS username and the ssh key it detected unless you said otherwise.
`up --no-provision` writes nothing.

Both write only the keys documented above. Keys they do not recognise are ignored when the file is read and dropped when
it is next written, so a misspelling neither fails nor survives — it silently does nothing.
