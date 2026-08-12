#!/usr/bin/env python3
"""Apply the provisioned display scale from inside the user's session.

First-boot provisioning stashes the profile's scale percentage in
``~/.config/bluefin-vm/scale-request``, and the ``bluefin-vm-apply-scale``
user oneshot runs this script at graphical-session start, gated on that
file. The work must happen in the session because the scales mutter accepts
are per-mode float32 doubles that only ``GetCurrentState`` reports, and
sending a value that is not bit-exact in that set aborts gnome-shell. The
request is therefore snapped to the nearest reported value, and only doubles
copied verbatim from mutter are ever transmitted.

The apply uses the temporary method, which is instant and dialog-free; the
persistent method raises a keep-or-revert dialog that reverts unattended.
Persistence comes from writing ``monitors.xml`` with the learned values,
which a later session start applies silently.

Exits with 0 when the scale was applied or there was nothing to do, 1 when
the request was invalid or refused (the request is consumed), and
``EX_TEMPFAIL`` when a transient failure left the request in place for the
next login to retry. The unit treats ``EX_TEMPFAIL`` as success so retries
stay quiet while real errors mark it failed.
"""

from __future__ import annotations

import logging
import os
import sys
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, ClassVar

# PyGObject is a guest-only dependency (the image build asserts it). The pure
# functions below are imported by host-side tests where gi is absent, so its
# absence must not break the import of this module. The type checker sees gi
# as Any -- its stubs package drags in a native PyGObject build.
if TYPE_CHECKING:
    Gio: Any
    GLib: Any
else:
    try:
        from gi.repository import Gio, GLib
    except ImportError:
        Gio = GLib = None

BUS_NAME = "org.gnome.Mutter.DisplayConfig"
OBJECT_PATH = "/org/gnome/Mutter/DisplayConfig"


@dataclass
class Mode:
    """Hold the single monitor's current mode from a GetCurrentState reply."""

    connector: str
    vendor: str
    product: str
    serial: str
    mode_id: str
    width: int
    height: int
    rate: float
    supported_scales: list[float]


class SdPrefixFormatter(logging.Formatter):
    """Format records with an sd-daemon severity prefix.

    systemd connects a service's streams to the journal and parses a leading
    ``<N>`` into the entry's priority, so the journal gets real severities
    without a python-systemd dependency. Run by hand, the prefixes appear
    literally.
    """

    _priorities: ClassVar[dict[int, int]] = {
        logging.DEBUG: 7,
        logging.INFO: 6,
        logging.WARNING: 4,
        logging.ERROR: 3,
        logging.CRITICAL: 2,
    }

    def format(self, record: logging.LogRecord) -> str:
        return f"<{self._priorities.get(record.levelno, 6)}>{record.getMessage()}"


def _make_logger() -> logging.Logger:
    """Build the module logger, with info on stdout and problems on stderr."""
    logger = logging.getLogger("bluefin-vm-apply-scale")
    logger.setLevel(logging.INFO)
    out = logging.StreamHandler(sys.stdout)
    out.addFilter(lambda record: record.levelno < logging.WARNING)
    err = logging.StreamHandler(sys.stderr)
    err.setLevel(logging.WARNING)
    for handler in (out, err):
        handler.setFormatter(SdPrefixFormatter())
        logger.addHandler(handler)
    return logger


log = _make_logger()


def read_request(path: str) -> int:
    """Read and validate the requested scale percentage.

    Args:
        path: The request file written by first-boot provisioning.

    Returns:
        The requested percentage.

    Raises:
        OSError: The file could not be read.
        ValueError: The content is not an integer between 50 and 400.
    """
    with open(path) as f:
        raw = f.read().strip()
    pct = int(raw)
    if not 50 <= pct <= 400:
        raise ValueError(f"{pct} is outside 50-400")
    return pct


def consume_request(path: str) -> None:
    """Ensure the request file is gone so it cannot trigger another run.

    Every caller has just read the file, and systemd runs this unit as a
    singleton, so nothing should remove it mid-run -- an already-gone file
    is unexpected and worth a warning, though the end state is still the
    one we wanted. Removal failures are logged, not raised.

    Args:
        path: The request file to remove.
    """
    try:
        os.remove(path)
    except FileNotFoundError:
        log.warning("request %s unexpectedly already gone", path)
    except OSError as e:
        log.warning("cannot remove %s: %s", path, e)


def current_mode(monitors: list) -> Mode | None:
    """Find the current mode of the single monitor.

    Args:
        monitors: The monitors element of an unpacked GetCurrentState reply.

    Returns:
        The current mode, or None when no mode is marked current (the
        session may still be settling).

    Raises:
        ValueError: There is not exactly one monitor.
    """
    if len(monitors) != 1:
        raise ValueError(f"expected one monitor, found {len(monitors)}")
    (connector, vendor, product, serial), modes, _ = monitors[0]
    for mode_id, width, height, rate, _, supported, props in modes:
        if props.get("is-current", False):
            return Mode(
                connector,
                vendor,
                product,
                serial,
                mode_id,
                width,
                height,
                rate,
                list(supported),
            )
    return None


def snap(supported: list[float], requested_pct: int) -> float | None:
    """Snap a requested percentage to the nearest supported scale.

    Args:
        supported: The mode's supported scales, verbatim from mutter.
        requested_pct: The requested percentage.

    Returns:
        A value from ``supported`` (never a computed one, since transmitting
        a fabricated double is the compositor-abort case), or None when the
        list is empty. Ties prefer the smaller scale.
    """
    if not supported:
        return None
    target = requested_pct / 100
    return min(supported, key=lambda s: (abs(s - target), s))


def monitors_xml(mode: Mode, scale: float) -> str:
    """Render monitors.xml as mutter itself writes it for one monitor.

    Args:
        mode: The mode to pin.
        scale: The snapped scale, written at full precision. The rate is
            written at three decimals, matching mutter's own output.

    Returns:
        The file content.
    """
    return f"""<monitors version="2">
  <configuration>
    <layoutmode>logical</layoutmode>
    <logicalmonitor>
      <x>0</x>
      <y>0</y>
      <scale>{scale!r}</scale>
      <primary>yes</primary>
      <monitor>
        <monitorspec>
          <connector>{mode.connector}</connector>
          <vendor>{mode.vendor}</vendor>
          <product>{mode.product}</product>
          <serial>{mode.serial}</serial>
        </monitorspec>
        <mode>
          <width>{mode.width}</width>
          <height>{mode.height}</height>
          <rate>{mode.rate:.3f}</rate>
        </mode>
      </monitor>
    </logicalmonitor>
  </configuration>
</monitors>
"""


def write_atomic(path: str, content: str) -> None:
    """Write a file via a temporary sibling and an atomic replace.

    Args:
        path: The destination path.
        content: The file content.
    """
    tmp = f"{path}.tmp"
    with open(tmp, "w") as f:
        f.write(content)
    os.replace(tmp, path)


def fetch_state(proxy, attempts: int = 30, delay: float = 1.0):
    """Poll GetCurrentState until mutter owns the DisplayConfig name.

    The unit starts with the session, which races gnome-shell; the name
    appears only once the shell is functional, so polling here beats unit
    ordering.

    Args:
        proxy: A DBus proxy for the DisplayConfig object.
        attempts: How many calls to attempt.
        delay: Seconds to sleep between attempts.

    Returns:
        The unpacked reply, or None after logging the final error.
    """
    error: Exception | None = None
    for _ in range(attempts):
        try:
            reply = proxy.call_sync(
                "GetCurrentState", None, Gio.DBusCallFlags.NONE, -1, None
            )
            return reply.unpack()
        except GLib.Error as e:
            error = e
            time.sleep(delay)
    # Log the exception itself: with attempts=0 there is no error to unpack,
    # and GLib.Error's str() carries the message anyway.
    log.error("DisplayConfig not reachable after %d attempts: %s", attempts, error)
    return None


def apply_temporary(
    proxy, serial: int, logical: tuple, mode: Mode, scale: float
) -> None:
    """Apply the scale with ApplyMonitorsConfig's temporary method.

    Args:
        proxy: A DBus proxy for the DisplayConfig object.
        serial: The state serial from GetCurrentState.
        logical: The unpacked current logical monitor, for its placement.
        mode: The current mode.
        scale: The snapped scale, a verbatim member of the supported set.

    Raises:
        GLib.Error: mutter refused the configuration.
    """
    x, y, _, transform, primary, *_ = logical
    config = GLib.Variant(
        "(uua(iiduba(ssa{sv}))a{sv})",
        (
            serial,
            1,  # The temporary method applies instantly with no dialog.
            [(x, y, scale, transform, primary, [(mode.connector, mode.mode_id, {})])],
            {},
        ),
    )
    proxy.call_sync("ApplyMonitorsConfig", config, Gio.DBusCallFlags.NONE, -1, None)


def main() -> int:
    """Run the request-to-applied pipeline.

    Returns:
        A process exit code as described in the module docstring.
    """
    config_home = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    request_path = os.path.join(config_home, "bluefin-vm", "scale-request")

    try:
        pct = read_request(request_path)
    except FileNotFoundError:
        log.info("no request present; nothing to do")
        return 0
    except (OSError, ValueError) as e:
        log.error("rejecting request %s: %s", request_path, e)
        consume_request(request_path)
        return 1

    if Gio is None:
        log.error("PyGObject is unavailable; the image build should have caught this")
        return 1

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    proxy = Gio.DBusProxy.new_sync(
        bus, Gio.DBusProxyFlags.NONE, None, BUS_NAME, OBJECT_PATH, BUS_NAME, None
    )
    state = fetch_state(proxy)
    if state is None:
        log.warning("keeping the request for the next login")
        return os.EX_TEMPFAIL

    serial, monitors, logical_monitors, _ = state
    try:
        mode = current_mode(monitors)
    except ValueError as e:
        log.error("unsupported display layout (%s); rejecting the request", e)
        consume_request(request_path)
        return 1
    if mode is None:
        log.warning("no mode is marked current; keeping the request for the next login")
        return os.EX_TEMPFAIL

    scale = snap(mode.supported_scales, pct)
    if scale is None:
        log.error(
            "mode %s reports no supported scales; rejecting the request", mode.mode_id
        )
        consume_request(request_path)
        return 1
    log.info("requested %d%% -> %r (mode %s)", pct, scale, mode.mode_id)

    live_scale = logical_monitors[0][2]
    if abs(live_scale - scale) < 1e-6:
        log.info("already at %r; skipping the live apply", scale)
    else:
        try:
            apply_temporary(proxy, serial, logical_monitors[0], mode, scale)
        except GLib.Error as e:
            log.error(
                "ApplyMonitorsConfig refused: %s; rejecting the request", e.message
            )
            consume_request(request_path)
            return 1

    write_atomic(os.path.join(config_home, "monitors.xml"), monitors_xml(mode, scale))
    consume_request(request_path)
    log.info("applied and persisted")
    return 0


if __name__ == "__main__":
    sys.exit(main())
