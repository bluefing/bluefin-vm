//! Interactive `tui` form -- a ratatui front-end over `core::config`.
//!
//! Thin by design: it edits a VM's profile fields and writes them back through
//! the same `Config` the CLI uses. No provisioning or Tart calls happen here;
//! `up` consumes what this saves.
//!
//! Inputs are typed (text / number / pick-a-value / toggle) so the form can
//! guide the user -- the ssh key is chosen from detected `~/.ssh/*.pub`, the
//! display from known resolutions -- rather than accepting free text it can't
//! check. Rendering lives in `ui`; the form logic is unit-tested.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::core::config::{Account, Config, Profile, Resources, Share, SshKey};

/// Common guest resolutions offered for the display (value uses Tart's `WxH`).
const DISPLAY_PRESETS: &[&str] = &["2560x1600", "2880x1800", "3024x1890", "3456x2160"];

// Field indices -- the form's fixed layout.
const F_USER: usize = 0;
const F_SSH: usize = 1;
const F_SUDO: usize = 2;
const F_SSHPW: usize = 3;
const F_DIR: usize = 4;
const F_READONLY: usize = 5;
const F_CPU: usize = 6;
const F_MEM: usize = 7;
const F_REFIT: usize = 8;
const F_DISPLAY: usize = 9;
const F_SCALE: usize = 10;

// Panel groupings, expressed via the same field-index constants so reordering
// or inserting a field can't desync a panel's range from the fields it shows.
const ACCOUNT_FIELDS: std::ops::Range<usize> = F_USER..F_DIR;
const SHARE_FIELDS: std::ops::Range<usize> = F_DIR..F_CPU;
const RESOURCES_FIELDS: std::ops::Range<usize> = F_CPU..F_SCALE + 1;

// Lively per-field accents (material 300-ish palette).
const A_USER: Color = Color::Rgb(0x4f, 0xc3, 0xf7); // light blue
const A_SSH: Color = Color::Rgb(0xff, 0xb3, 0x74); // orange
const A_SUDO: Color = Color::Rgb(0x81, 0xc7, 0x84); // green
const A_SSHPW: Color = Color::Rgb(0xff, 0x8a, 0x80); // salmon
const A_DIR: Color = Color::Rgb(0xf0, 0x6e, 0x8e); // rose
const A_READONLY: Color = Color::Rgb(0x4d, 0xb6, 0xac); // teal-green
const A_CPU: Color = Color::Rgb(0xff, 0xd5, 0x4f); // amber
const A_MEM: Color = Color::Rgb(0xce, 0x93, 0xd8); // purple
const A_REFIT: Color = Color::Rgb(0x90, 0xa4, 0xae); // blue-grey
const A_DISPLAY: Color = Color::Rgb(0x4d, 0xd0, 0xe1); // teal
const A_SCALE: Color = Color::Rgb(0xd4, 0xe1, 0x57); // lime

/// One selectable option in a `Choice`. `value` is what gets stored; `None`
/// means "unset" (the `(none)` / `(default)` entry).
struct Opt {
    label: String,
    value: Option<String>,
}

enum Input {
    /// Free text; `placeholder` shows when empty.
    Text {
        value: String,
        placeholder: &'static str,
    },
    /// Digit-only text; `default` shows as the placeholder when empty. Bounded
    /// by `min..=max` (host capacity), checked on save.
    Number {
        value: String,
        default: u32,
        min: u32,
        max: u32,
    },
    /// Pick one of a fixed list, cycled with left/right.
    Choice { options: Vec<Opt>, selected: usize },
    /// On/off, flipped with space.
    Toggle { on: bool },
}

struct Field {
    label: &'static str,
    accent: Color,
    /// What the selected field expects, shown in the status line.
    hint: String,
    input: Input,
}

struct Form {
    name: String,
    fields: Vec<Field>,
    selected: usize,
    error: Option<String>,
}

/// Open the form for VM `name`, editing its saved profile; save on confirm.
pub fn run(name: &str) -> Result<()> {
    let mut config = Config::load()?;
    let mut form = Form::build(name, config.profile(name), &detected_ssh_keys());

    let mut terminal = ratatui::init();
    let saved = event_loop(&mut terminal, &mut form);
    ratatui::restore();

    if saved? {
        form.apply_to(&mut config);
        config.save()?;
        eprintln!("Saved profile '{name}' to {}", Config::path()?.display());
    } else {
        eprintln!("Cancelled -- no changes saved.");
    }
    Ok(())
}

impl Form {
    /// Build the form for `name` from its `profile` (if any) and the host's
    /// detected ssh keys (passed in so the logic stays testable).
    fn build(name: &str, profile: Option<&Profile>, ssh_keys: &[PathBuf]) -> Self {
        let account = profile.map(|p| &p.account);
        let share = profile.map(|p| &p.share);
        let resources = profile.map(|p| &p.resources);

        let cpu_max = host_cpus();
        let mem_mib = host_mem_mib();
        let mem_gib_max = if mem_mib == u32::MAX {
            u32::MAX
        } else {
            mem_mib / 1024
        };
        let fields = vec![
            Field {
                label: "Username",
                accent: A_USER,
                hint: "lowercase letter or _, then [a-z0-9_-], max 32 (blank = host $USER)".into(),
                input: Input::Text {
                    value: account.and_then(|a| a.user.clone()).unwrap_or_default(),
                    placeholder: "host $USER",
                },
            },
            Field {
                label: "SSH key",
                accent: A_SSH,
                hint: "public key installed for ssh -- ←/→ to choose".into(),
                input: ssh_choice(ssh_keys, account.and_then(|a| a.ssh_key.as_ref())),
            },
            Field {
                label: "Sudo password",
                accent: A_SUDO,
                hint: "sudo asks for your login password; off = passwordless \
                       (no prompt) -- ←/→ or space"
                    .into(),
                input: Input::Toggle {
                    on: account.and_then(|a| a.sudo_password).unwrap_or(true),
                },
            },
            Field {
                label: "SSH password",
                accent: A_SSHPW,
                hint: "allow ssh password login; off = key-only (e.g. a bridged VM) \
                       -- ←/→ or space"
                    .into(),
                input: Input::Toggle {
                    on: account.and_then(|a| a.ssh_password_auth).unwrap_or(true),
                },
            },
            Field {
                label: "Directory",
                accent: A_DIR,
                hint: "host folder mounted as bluefin-share (blank = ~/bluefin-share)".into(),
                input: Input::Text {
                    value: share
                        .and_then(|s| s.directory.as_ref())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    placeholder: "~/bluefin-share",
                },
            },
            Field {
                label: "Read-only",
                accent: A_READONLY,
                hint: "mount the share read-only in the guest -- ←/→ or space".into(),
                input: Input::Toggle {
                    on: share.and_then(|s| s.read_only).unwrap_or(false),
                },
            },
            Field {
                label: "CPU",
                accent: A_CPU,
                hint: format!("virtual cores, 1–{cpu_max} -- ←/→ or type"),
                input: Input::Number {
                    value: num(resources.and_then(|r| r.cpu)),
                    default: 4,
                    min: 1,
                    max: cpu_max,
                },
            },
            Field {
                label: "Memory (GiB)",
                accent: A_MEM,
                hint: mem_hint(mem_gib_max),
                input: Input::Number {
                    value: num(resources.and_then(|r| r.memory_mib).map(|m| m / 1024)),
                    default: 4,
                    min: 1,
                    max: mem_gib_max,
                },
            },
            Field {
                label: "Refit",
                accent: A_REFIT,
                hint: "auto-resize the guest to the window; turn off to set a fixed \
                       resolution + scale -- ←/→ or space"
                    .into(),
                input: Input::Toggle {
                    on: resources.and_then(|r| r.refit).unwrap_or(true),
                },
            },
            Field {
                label: "Display",
                accent: A_DISPLAY,
                hint: "guest resolution (needs Refit off) -- ←/→ to choose".into(),
                input: display_choice(resources.and_then(|r| r.display.as_deref())),
            },
            Field {
                label: "Scale",
                accent: A_SCALE,
                hint:
                    "guest scale target, snapped to the nearest the display supports (needs Refit off) -- ←/→"
                        .into(),
                input: scale_choice(resources.and_then(|r| r.scale)),
            },
        ];

        Self {
            name: name.to_string(),
            fields,
            selected: 0,
            error: None,
        }
    }

    /// Parse the fields back into the VM's profile, preserving anything the form
    /// doesn't cover.
    fn apply_to(&self, config: &mut Config) {
        // The form always commits an explicit ssh-key choice on save: a path,
        // or "(none)" -- which must persist as `Disabled`, not `None`, or a
        // later resolve would treat it as unset and auto-detect a key anyway.
        let account = Account {
            user: self.text(F_USER),
            ssh_key: Some(match self.choice(F_SSH) {
                Some(s) => SshKey::Path(PathBuf::from(s)),
                None => SshKey::Disabled,
            }),
            sudo_password: Some(self.toggle(F_SUDO)),
            ssh_password_auth: Some(self.toggle(F_SSHPW)),
        };
        let share = Share {
            directory: self.text(F_DIR).map(PathBuf::from),
            read_only: Some(self.toggle(F_READONLY)),
        };
        // Refit and a fixed resolution/scale are mutually exclusive: with refit
        // on, a saved display/scale can't hold, so don't persist them -- keep
        // the config honest about what actually applies.
        let refit = self.toggle(F_REFIT);
        let resources = Resources {
            cpu: self.number(F_CPU),
            memory_mib: self.number(F_MEM).map(|gib| gib.saturating_mul(1024)),
            display: (!refit).then(|| self.choice(F_DISPLAY)).flatten(),
            scale: (!refit)
                .then(|| self.choice(F_SCALE).and_then(|s| s.parse().ok()))
                .flatten(),
            refit: Some(refit),
        };
        let mut profile = config.profile(&self.name).cloned().unwrap_or_default();
        profile.account = account;
        profile.share = share;
        profile.resources = resources;
        config.set_profile(&self.name, profile);
    }

    /// Save is blocked while this returns `Err` (message shown to the user).
    fn validate(&self) -> Result<(), String> {
        if let Some(user) = self.text(F_USER) {
            if !crate::core::provision::valid_username(&user) {
                return Err(format!(
                    "Invalid username '{user}': lowercase letter or _, then [a-z0-9_-], max 32"
                ));
            }
        }
        for field in &self.fields {
            let Input::Number {
                value, min, max, ..
            } = &field.input
            else {
                continue;
            };
            let v = value.trim();
            if v.is_empty() {
                continue; // blank -> the built-in default is used
            }
            let n: u32 = v
                .parse()
                .map_err(|_| format!("{} must be a whole number", field.label))?;
            if n < *min || n > *max {
                return Err(if *max == u32::MAX {
                    format!("{} must be at least {min} (got {n})", field.label)
                } else {
                    format!("{} must be {min}–{max} (got {n})", field.label)
                });
            }
        }
        Ok(())
    }

    // --- field readers ------------------------------------------------------

    fn text(&self, i: usize) -> Option<String> {
        match &self.fields[i].input {
            Input::Text { value, .. } => {
                let v = value.trim();
                (!v.is_empty()).then(|| v.to_string())
            }
            _ => None,
        }
    }
    fn number(&self, i: usize) -> Option<u32> {
        match &self.fields[i].input {
            Input::Number { value, .. } => value.trim().parse().ok(),
            _ => None,
        }
    }
    fn choice(&self, i: usize) -> Option<String> {
        match &self.fields[i].input {
            Input::Choice { options, selected } => options[*selected].value.clone(),
            _ => None,
        }
    }
    fn toggle(&self, i: usize) -> bool {
        matches!(&self.fields[i].input, Input::Toggle { on: true })
    }

    /// Display and Scale only apply with Refit off, so they're greyed and
    /// unselectable while it's on -- setting them then would be a lie.
    fn disabled(&self, i: usize) -> bool {
        matches!(i, F_DISPLAY | F_SCALE) && self.toggle(F_REFIT)
    }

    // --- navigation + editing ----------------------------------------------

    fn next(&mut self) {
        let n = self.fields.len();
        for step in 1..=n {
            let i = (self.selected + step) % n;
            if !self.disabled(i) {
                self.selected = i;
                return;
            }
        }
    }
    fn prev(&mut self) {
        let n = self.fields.len();
        for step in 1..=n {
            let i = (self.selected + n - step) % n;
            if !self.disabled(i) {
                self.selected = i;
                return;
            }
        }
    }

    fn edit(&mut self, code: KeyCode) {
        if self.disabled(self.selected) {
            return; // a disabled field ignores input (belt-and-braces with nav)
        }
        match &mut self.fields[self.selected].input {
            Input::Text { value, .. } => match code {
                KeyCode::Char(c) => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                _ => {}
            },
            Input::Number {
                value,
                default,
                min,
                max,
            } => match code {
                KeyCode::Char(c) if c.is_ascii_digit() => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                // ←/→ step the number, clamped to the field's range.
                KeyCode::Left => {
                    let n = value.parse::<u32>().unwrap_or(*default);
                    *value = n.saturating_sub(1).max(*min).to_string();
                }
                KeyCode::Right => {
                    let n = value.parse::<u32>().unwrap_or(*default);
                    *value = n.saturating_add(1).min(*max).to_string();
                }
                _ => {}
            },
            Input::Toggle { on } => {
                // A bool is a two-state choice: space, ←, or → all flip it.
                if matches!(code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                    *on = !*on;
                }
            }
            Input::Choice { options, selected } => {
                let n = options.len();
                match code {
                    KeyCode::Left => *selected = (*selected + n - 1) % n,
                    KeyCode::Right | KeyCode::Char(' ') => *selected = (*selected + 1) % n,
                    _ => {}
                }
            }
        }
    }
}

/// Build the ssh-key choice: each detected key, plus `(none)`. Pre-selects the
/// saved key (adding it if it's no longer on disk), the saved `(none)` if the
/// key was explicitly disabled, else the first key found.
fn ssh_choice(keys: &[PathBuf], saved: Option<&SshKey>) -> Input {
    let mut options: Vec<Opt> = keys
        .iter()
        .map(|p| Opt {
            label: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            value: Some(p.to_string_lossy().into_owned()),
        })
        .collect();

    // Ensure a saved path is present so re-opening never silently drops it.
    let saved_path = match saved {
        Some(SshKey::Path(p)) => Some(p.to_string_lossy().into_owned()),
        _ => None,
    };
    if let Some(s) = &saved_path {
        if !options
            .iter()
            .any(|o| o.value.as_deref() == Some(s.as_str()))
        {
            options.insert(
                0,
                Opt {
                    label: format!("{s} (missing)"),
                    value: Some(s.clone()),
                },
            );
        }
    }
    options.push(Opt {
        label: "(none)".into(),
        value: None,
    });

    let selected = match saved {
        Some(SshKey::Path(_)) => options
            .iter()
            .position(|o| o.value.as_deref() == saved_path.as_deref())
            .unwrap_or(0),
        Some(SshKey::Disabled) => options.len() - 1, // "(none)"
        None if options.len() > 1 => 0,              // first detected key
        None => options.len() - 1,                   // only "(none)"
    };
    Input::Choice { options, selected }
}

/// Build the scale choice: GNOME's standard fractional ladder -- 100% (its own
/// default), 125, 150, 175, 200. The value is a target: at first login the
/// guest snaps it to the nearest scale mutter supports for the live mode
/// (which steps exist varies per resolution). Keeps any other saved value so
/// re-opening never drops it.
fn scale_choice(saved: Option<u32>) -> Input {
    let mut options = vec![Opt {
        label: "100% (default)".into(),
        value: None,
    }];
    for pct in ["125", "150", "175", "200"] {
        options.push(Opt {
            label: format!("{pct}%"),
            value: Some(pct.into()),
        });
    }
    let saved_str = saved.map(|pct| pct.to_string());
    if let Some(s) = &saved_str {
        if !options
            .iter()
            .any(|o| o.value.as_deref() == Some(s.as_str()))
        {
            options.push(Opt {
                label: format!("{s}%"),
                value: Some(s.clone()),
            });
        }
    }
    let selected = saved_str
        .as_deref()
        .and_then(|s| options.iter().position(|o| o.value.as_deref() == Some(s)))
        .unwrap_or(0);
    Input::Choice { options, selected }
}

/// Build the display choice: `(default)` plus preset resolutions, keeping any
/// saved custom value.
fn display_choice(saved: Option<&str>) -> Input {
    let mut options = vec![Opt {
        label: "1920×1200 (default)".into(),
        value: None,
    }];
    for r in DISPLAY_PRESETS {
        options.push(Opt {
            label: r.replace('x', "×"),
            value: Some((*r).to_string()),
        });
    }
    if let Some(s) = saved {
        if !options.iter().any(|o| o.value.as_deref() == Some(s)) {
            options.push(Opt {
                label: s.replace('x', "×"),
                value: Some(s.to_string()),
            });
        }
    }
    let selected = saved
        .and_then(|s| options.iter().position(|o| o.value.as_deref() == Some(s)))
        .unwrap_or(0);
    Input::Choice { options, selected }
}

fn detected_ssh_keys() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let ssh = PathBuf::from(home).join(".ssh");
    let mut keys: Vec<PathBuf> = std::fs::read_dir(ssh)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pub"))
        .collect();
    keys.sort();
    keys
}

fn num(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

/// Host logical CPU count -- the ceiling for the VM's vCPUs (Virtualization.
/// framework can't oversubscribe cores). Falls back to 8 if it can't be read.
fn host_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(8)
}

/// Host RAM in MiB -- the ceiling for the VM's memory. `u32::MAX` (no cap) if it
/// can't be read, so a stranger host never wrongly rejects a value.
fn host_mem_mib() -> u32 {
    std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|bytes| (bytes / (1024 * 1024)).min(u32::MAX as u64) as u32)
        .unwrap_or(u32::MAX)
}

fn mem_hint(gib_max: u32) -> String {
    if gib_max == u32::MAX {
        "RAM in GiB -- ←/→ or type".into()
    } else {
        format!("RAM in GiB, 1–{gib_max} -- ←/→ or type")
    }
}

/// Returns `Ok(true)` on save (Enter, if valid), `Ok(false)` on cancel (Esc).
fn event_loop(terminal: &mut DefaultTerminal, form: &mut Form) -> Result<bool> {
    let start = Instant::now();
    let tick = Duration::from_millis(80);
    loop {
        let phase = start.elapsed().as_millis() as u64;
        terminal
            .draw(|frame| ui(frame, form, phase, None))
            .context("drawing")?;

        // Poll rather than block, so the selected row keeps breathing while idle.
        if !event::poll(tick).context("polling input")? {
            continue;
        }
        let Event::Key(key) = event::read().context("reading input")? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue; // ignore key-release / repeat on platforms that send them
        }
        form.error = None; // any keypress clears a stale validation error
        match key.code {
            KeyCode::Esc => return Ok(false),
            KeyCode::Enter => match form.validate() {
                Ok(()) => {
                    // Green confirmation flash before the terminal is restored.
                    // Polling (rather than a blocking sleep) keeps redrawing and
                    // draining input for the flash's duration instead of
                    // freezing the last frame while ignoring the terminal.
                    let msg = format!("✓ saved profile '{}'", form.name);
                    let flash_until = Instant::now() + Duration::from_millis(320);
                    loop {
                        let phase = start.elapsed().as_millis() as u64;
                        terminal
                            .draw(|frame| ui(frame, form, phase, Some(&msg)))
                            .context("drawing")?;
                        let remaining = flash_until.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        if event::poll(remaining).context("polling input")? {
                            event::read().context("reading input")?;
                        }
                    }
                    return Ok(true);
                }
                Err(msg) => form.error = Some(msg),
            },
            KeyCode::Up => form.prev(),
            KeyCode::Down | KeyCode::Tab => form.next(),
            other => form.edit(other),
        }
    }
}

// --- rendering --------------------------------------------------------------

// Muted-but-readable grey for placeholders, the hint line, and secondary text --
// DarkGray is too low-contrast on a near-black background.
const DIM: Style = Style::new().fg(Color::Rgb(0x9a, 0xa1, 0xab));
const OK: Color = Color::Rgb(0x81, 0xc7, 0x84);
const BAD: Color = Color::Rgb(0xef, 0x53, 0x50);

fn ui(frame: &mut Frame, form: &Form, phase: u64, flash: Option<&str>) {
    let [top, status, controls] = Layout::vertical([
        Constraint::Length(20),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // Form on the left; Dakota keeps you company on the right when there's room.
    let (formcol, mascot) = if top.width >= 90 {
        let [left, right] =
            Layout::horizontal([Constraint::Min(46), Constraint::Length(42)]).areas(top);
        (left, Some(right))
    } else {
        (top, None)
    };
    let [title, body, _] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(17),
        Constraint::Min(0),
    ])
    .areas(formcol);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("bluefin-vm", Style::new().fg(A_DISPLAY).bold()),
            Span::styled(" tui", Style::new().fg(Color::White).bold()),
            Span::styled(format!("  ·  {}", form.name), DIM),
        ]))
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(A_USER)),
        ),
        title,
    );

    let [account, share, resources] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(7),
    ])
    .areas(body);

    // Three rounded, coloured panels; borders flash green on save.
    let (acc_border, share_border, res_border) = match flash {
        Some(_) => (OK, OK, OK),
        None => (A_USER, A_DIR, A_MEM),
    };
    panel(
        frame,
        account,
        " account ",
        acc_border,
        form,
        ACCOUNT_FIELDS,
        phase,
    );
    panel(
        frame,
        share,
        " share ",
        share_border,
        form,
        SHARE_FIELDS,
        phase,
    );
    panel(
        frame,
        resources,
        " resources ",
        res_border,
        form,
        RESOURCES_FIELDS,
        phase,
    );
    if let Some(area) = mascot {
        dakota(frame, area, flash.is_some());
    }

    let status_line = match (flash, &form.error) {
        (Some(msg), _) => Line::from(Span::styled(format!("  {msg}"), Style::new().fg(OK).bold())),
        (None, Some(err)) => Line::from(Span::styled(
            format!("  ✖ {err}"),
            Style::new().fg(BAD).bold(),
        )),
        (None, None) => Line::from(Span::styled(
            format!("  {}", form.fields[form.selected].hint),
            DIM,
        )),
    };
    frame.render_widget(Paragraph::new(status_line), status);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "  ↑/↓ field · ←/→ adjust · space toggle · type to edit · Enter save · Esc cancel",
            DIM,
        )),
        controls,
    );
}

/// Render a titled, rounded panel for the fields in `range`, highlighting the
/// globally-selected row.
fn panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    border: Color,
    form: &Form,
    range: std::ops::Range<usize>,
    phase: u64,
) {
    let rows: Vec<ListItem> = range
        .map(|i| row(&form.fields[i], i == form.selected, form.disabled(i), phase))
        .collect();
    frame.render_widget(
        List::new(rows).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border))
                .title(Span::styled(
                    title.to_string(),
                    Style::new().fg(border).bold(),
                )),
        ),
        area,
    );
}

/// Dakota the Bluefin chicken -- a small mascot that cheeps "saved!" on write.
/// Swap the art for the real Dakota anytime; it's just these lines.
/// Dakota the chonky Dakosaurus -- Bluefin's mascot, from a tiny raw blob
/// (`width:u16 height:u16` then RGBA). Rendered as half-block cells, so no image
/// decoder or terminal graphics protocol is needed -- just truecolour.
const DAKOTA_RGBA: &[u8] = include_bytes!("dakota.rgba");

fn dakota(frame: &mut Frame, area: Rect, flash: bool) {
    let (title, accent) = if flash {
        (" dakota · saved! ", OK)
    } else {
        (" dakota ", A_DISPLAY)
    };
    frame.render_widget(
        Paragraph::new(half_block_art(DAKOTA_RGBA)).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(scale(accent, 0.6)))
                .title(Span::styled(title, Style::new().fg(accent).bold())),
        ),
        area,
    );
}

/// Decode a `width:u16 height:u16` + RGBA blob into half-block `▀`/`▄` lines:
/// each cell packs two vertical pixels via fg (top) / bg (bottom); a transparent
/// half falls through to the terminal background.
fn half_block_art(blob: &[u8]) -> Vec<Line<'static>> {
    let w = u16::from_le_bytes([blob[0], blob[1]]) as usize;
    let h = u16::from_le_bytes([blob[2], blob[3]]) as usize;
    let px = &blob[4..];
    let at = |x: usize, y: usize| -> Option<Color> {
        if y >= h {
            return None;
        }
        let i = (y * w + x) * 4;
        (px[i + 3] >= 128).then(|| Color::Rgb(px[i], px[i + 1], px[i + 2]))
    };
    (0..h.div_ceil(2))
        .map(|row| {
            let spans: Vec<Span<'static>> = (0..w)
                .map(|x| match (at(x, row * 2), at(x, row * 2 + 1)) {
                    (Some(t), Some(b)) => Span::styled("▀", Style::new().fg(t).bg(b)),
                    (Some(t), None) => Span::styled("▀", Style::new().fg(t)),
                    (None, Some(b)) => Span::styled("▄", Style::new().fg(b)),
                    (None, None) => Span::raw(" "),
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// One field row: a pulsing marker + coloured icon/label, then the input control.
/// A `disabled` row (a gated Display/Scale while Refit is on) renders flat grey
/// and can't be selected.
fn row(field: &Field, selected: bool, disabled: bool, phase: u64) -> ListItem<'static> {
    // The selected row's accent breathes; the rest sit at a steady tint; a
    // disabled row drops to grey so it reads as inert.
    let color = if disabled {
        Color::DarkGray
    } else if selected {
        pulse(field.accent, phase)
    } else {
        scale(field.accent, 0.72)
    };
    let mut label_style = Style::new().fg(color);
    if selected {
        label_style = label_style.add_modifier(Modifier::BOLD);
    }
    let mut spans = vec![
        Span::styled(if selected { "❯ " } else { "  " }, Style::new().fg(color)),
        Span::styled(format!("{:<14}", field.label), label_style),
    ];
    spans.extend(control(&field.input, selected, field.accent, disabled));

    let bg = if selected {
        Style::new().bg(Color::Indexed(236))
    } else {
        Style::new()
    };
    ListItem::new(Line::from(spans)).style(bg)
}

/// The value/control spans for an input (caret + placeholder for editables,
/// `‹ … ›` for choices in the field's accent, a glyph for the toggle). A
/// `disabled` control renders flat grey with no accent.
fn control(input: &Input, selected: bool, accent: Color, disabled: bool) -> Vec<Span<'static>> {
    let caret = || Span::styled("▏", Style::new().fg(accent));
    match input {
        Input::Text { value, placeholder } => editable(value, placeholder, selected, caret()),
        Input::Number { value, default, .. } => {
            let ph = format!("default {default}");
            editable(value, &ph, selected, caret())
        }
        Input::Toggle { on } => vec![if *on {
            Span::styled("● on", Style::new().fg(OK).bold())
        } else {
            Span::styled("○ off", DIM)
        }],
        Input::Choice {
            options,
            selected: sel,
        } => {
            let arrows = Style::new().fg(if disabled {
                Color::DarkGray
            } else if selected {
                accent
            } else {
                Color::DarkGray
            });
            let value = Style::new().fg(if disabled {
                Color::DarkGray
            } else if selected {
                Color::White
            } else {
                Color::Gray
            });
            vec![
                Span::styled("‹ ", arrows),
                Span::styled(options[*sel].label.clone(), value),
                Span::styled(" ›", arrows),
            ]
        }
    }
}

fn editable(
    value: &str,
    placeholder: &str,
    selected: bool,
    caret: Span<'static>,
) -> Vec<Span<'static>> {
    let mut spans = if value.is_empty() {
        vec![Span::styled(format!("<{placeholder}>"), DIM)]
    } else {
        vec![Span::styled(
            value.to_string(),
            Style::new().fg(Color::White),
        )]
    };
    if selected {
        spans.push(caret);
    }
    spans
}

// --- animation helpers ------------------------------------------------------

/// Scale an RGB colour's brightness by `f` (named colours pass through).
fn scale(color: Color, f: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * f) as u8,
            (g as f64 * f) as u8,
            (b as f64 * f) as u8,
        ),
        other => other,
    }
}

/// Breathe an accent between ~60% and 100% brightness on a ~1.2s cycle.
fn pulse(color: Color, phase: u64) -> Color {
    let t = (phase as f64 / 190.0).sin() * 0.5 + 0.5; // 0..1
    scale(color, 0.6 + 0.4 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/home/me/.ssh/id_ed25519.pub"),
            PathBuf::from("/home/me/.ssh/id_rsa.pub"),
        ]
    }

    #[test]
    fn build_populates_and_preselects_saved_choices() {
        let profile = Profile {
            account: Account {
                user: Some("alice".into()),
                ssh_key: Some(SshKey::Path("/home/me/.ssh/id_rsa.pub".into())),
                sudo_password: Some(false),
                ssh_password_auth: Some(false),
            },
            share: Share {
                directory: Some("/home/me/projects".into()),
                read_only: Some(true),
            },
            resources: Resources {
                cpu: Some(8),
                memory_mib: None,
                display: Some("2560x1600".into()),
                scale: Some(200),
                refit: Some(false), // a fixed-resolution profile
            },
        };
        let form = Form::build("work", Some(&profile), &keys());
        assert_eq!(form.text(F_USER).as_deref(), Some("alice"));
        assert_eq!(
            form.choice(F_SSH).as_deref(),
            Some("/home/me/.ssh/id_rsa.pub")
        );
        assert!(!form.toggle(F_SUDO)); // saved passwordless -> "Sudo password" off
        assert!(!form.toggle(F_SSHPW)); // saved ssh-password-off round-trips
        assert_eq!(form.text(F_DIR).as_deref(), Some("/home/me/projects"));
        assert!(form.toggle(F_READONLY));
        assert_eq!(form.number(F_CPU), Some(8));
        assert_eq!(form.number(F_MEM), None); // unset -> blank
        assert!(!form.toggle(F_REFIT)); // saved refit-off round-trips
        assert_eq!(form.choice(F_DISPLAY).as_deref(), Some("2560x1600"));
        assert_eq!(form.choice(F_SCALE).as_deref(), Some("200"));
    }

    #[test]
    fn fresh_form_preselects_first_key_and_default_display() {
        let form = Form::build("v", None, &keys());
        // first detected key; sudo password on (prompts) and ssh password on by
        // default; display default (None).
        assert_eq!(
            form.choice(F_SSH).as_deref(),
            Some("/home/me/.ssh/id_ed25519.pub")
        );
        assert!(form.toggle(F_SUDO));
        assert!(form.toggle(F_SSHPW));
        assert_eq!(form.choice(F_DISPLAY), None);
    }

    #[test]
    fn editing_is_typed_per_input() {
        let mut form = Form::build("v", None, &keys());

        form.selected = F_USER; // text: appends
        for c in ['b', 'o', 'b'] {
            form.edit(KeyCode::Char(c));
        }
        assert_eq!(form.text(F_USER).as_deref(), Some("bob"));

        form.selected = F_CPU; // number: digits only
        for c in ['1', 'x', '6'] {
            form.edit(KeyCode::Char(c));
        }
        assert_eq!(form.number(F_CPU), Some(16));

        form.selected = F_SUDO; // toggle: space and ←/→ all flip it (default on)
        assert!(form.toggle(F_SUDO));
        form.edit(KeyCode::Char(' '));
        assert!(!form.toggle(F_SUDO));
        form.edit(KeyCode::Right);
        assert!(form.toggle(F_SUDO));
        form.edit(KeyCode::Left);
        assert!(!form.toggle(F_SUDO));

        form.selected = F_SSH; // choice: right cycles, and wraps to "(none)"
        form.edit(KeyCode::Right); // -> id_rsa
        assert_eq!(
            form.choice(F_SSH).as_deref(),
            Some("/home/me/.ssh/id_rsa.pub")
        );
        form.edit(KeyCode::Right); // -> (none)
        assert_eq!(form.choice(F_SSH), None);
    }

    #[test]
    fn invalid_username_blocks_save() {
        let mut form = Form::build("v", None, &keys());
        form.selected = F_USER;
        for c in ['B', 'a', 'd'] {
            form.edit(KeyCode::Char(c)); // capital -> invalid
        }
        assert!(form.validate().is_err());
        // A valid name passes.
        let mut ok = Form::build("v", None, &keys());
        ok.selected = F_USER;
        for c in ['g', 'o', 'o', 'd'] {
            ok.edit(KeyCode::Char(c));
        }
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn out_of_range_number_blocks_save() {
        let mut form = Form::build("v", None, &keys());
        form.selected = F_CPU;
        for c in ['9', '9', '9', '9', '9'] {
            form.edit(KeyCode::Char(c)); // 99999 cores -- beyond any host
        }
        assert!(form.validate().is_err());
        // Clearing it (blank -> built-in default) validates fine.
        for _ in 0..5 {
            form.edit(KeyCode::Backspace);
        }
        assert!(form.validate().is_ok());
    }

    #[test]
    fn memory_is_gib_in_the_form_but_mib_in_config() {
        // Loading shows GiB...
        let profile = Profile {
            resources: Resources {
                memory_mib: Some(8192),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            Form::build("v", Some(&profile), &keys()).number(F_MEM),
            Some(8)
        );

        // ...and saving GiB writes MiB.
        let mut form = Form::build("v", None, &keys());
        form.selected = F_MEM;
        form.edit(KeyCode::Char('4'));
        let mut config = Config::default();
        form.apply_to(&mut config);
        assert_eq!(
            config.profile("v").unwrap().resources.memory_mib,
            Some(4096)
        );
    }

    #[test]
    fn apply_to_writes_choices_and_treats_blanks_as_unset() {
        let mut form = Form::build("work", None, &keys());
        form.selected = F_USER;
        for c in ['c', 'a', 'r', 'o', 'l'] {
            form.edit(KeyCode::Char(c));
        }
        form.selected = F_SSH; // -> (none)
        form.edit(KeyCode::Left);
        form.selected = F_DIR; // type a host share path
        for c in "/tmp/share".chars() {
            form.edit(KeyCode::Char(c));
        }
        form.selected = F_READONLY; // flip read-only on
        form.edit(KeyCode::Char(' '));
        form.selected = F_REFIT; // turn refit off so display/scale apply
        form.edit(KeyCode::Char(' '));
        form.selected = F_DISPLAY; // -> first preset
        form.edit(KeyCode::Right);
        form.selected = F_SCALE; // 100% -> 125% -> 150%
        form.edit(KeyCode::Right);
        form.edit(KeyCode::Right);

        let mut config = Config::default();
        form.apply_to(&mut config);
        let p = config.profile("work").unwrap();
        assert_eq!(p.account.user.as_deref(), Some("carol"));
        // Explicitly disabled, not merely unset -- resolve() must not
        // auto-detect a key on top of this.
        assert_eq!(p.account.ssh_key, Some(SshKey::Disabled));
        assert_eq!(p.share.directory, Some(PathBuf::from("/tmp/share")));
        assert_eq!(p.share.read_only, Some(true));
        assert_eq!(p.resources.cpu, None); // never touched -> unset
        assert_eq!(p.resources.refit, Some(false));
        assert_eq!(p.resources.display.as_deref(), Some("2560x1600"));
        assert_eq!(p.resources.scale, Some(150));
    }

    #[test]
    fn refit_on_gates_out_display_and_scale() {
        let form = Form::build("v", None, &keys());
        // Refit defaults on, so Display/Scale are disabled and skipped.
        assert!(form.toggle(F_REFIT));
        assert!(form.disabled(F_DISPLAY));
        assert!(form.disabled(F_SCALE));

        // Even if a display/scale value is somehow selected, refit-on drops
        // them on save -- they can't apply while the resolution is dynamic.
        let mut config = Config::default();
        form.apply_to(&mut config);
        let p = config.profile("v").unwrap();
        assert_eq!(p.resources.refit, Some(true));
        assert_eq!(p.resources.display, None);
        assert_eq!(p.resources.scale, None);
    }

    #[test]
    fn navigation_skips_disabled_fields() {
        let mut form = Form::build("v", None, &keys()); // refit on
        form.selected = F_MEM;
        form.next(); // F_MEM -> F_REFIT (skips nothing yet)
        assert_eq!(form.selected, F_REFIT);
        form.next(); // F_REFIT -> wraps past disabled Display/Scale to F_USER
        assert_eq!(form.selected, F_USER);
    }
}
