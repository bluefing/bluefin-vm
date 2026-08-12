"""Unit tests for the pure half of image/apply-scale.py.

The supported-scales arrays are real captures from mutter's GetCurrentState
in the guest (GNOME Shell 50, virtio-gpu) -- irregular per mode and made of
float32-precision doubles, which is the whole reason snapping exists. The
invariant that matters most is that snap returns a value from the array,
never a computed one, because transmitting a fabricated double aborts
gnome-shell.

The pytest pre-commit hook runs this file with a pinned pytest in a venv
pre-commit manages, so nothing is assumed installed on the host.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).parents[2] / "image" / "apply-scale.py"
spec = importlib.util.spec_from_file_location("apply_scale", SCRIPT)
assert spec is not None and spec.loader is not None, SCRIPT
apply_scale = importlib.util.module_from_spec(spec)
sys.modules["apply_scale"] = apply_scale  # dataclasses resolves annotations here
spec.loader.exec_module(apply_scale)

# Captured supported-scales, full precision, from GetCurrentState.
M1920 = [1.0, 1.25, 1.3333333730697632, 1.5, 1.6666666269302368, 2.0]
M2560 = [
    1.0,
    1.25,
    1.3333333730697632,
    1.6666666269302368,
    2.0,
    2.5,
    2.6666667461395264,
]
M2048 = [1.0, 1.3333333730697632, 2.0]
M1366 = [1.0]

# A monitors structure shaped like GetCurrentState's unpacked reply.
MONITORS = [
    (
        ("Virtual-1", "unknown", "unknown", "unknown"),
        [
            ("2560x1600@59.987", 2560, 1600, 59.987, 1.0, M2560, {}),
            ("2048x1152@60.000", 2048, 1152, 60.0, 1.0, M2048, {"is-current": True}),
        ],
        {},
    )
]


def test_snap_150_is_exact_at_1920x1200():
    assert apply_scale.snap(M1920, 150) == 1.5


def test_snap_150_at_2560x1600_ties_prefer_the_smaller_scale():
    assert apply_scale.snap(M2560, 150) == 1.3333333730697632


def test_snap_integer_targets_land_on_the_integer_scales():
    assert apply_scale.snap(M2560, 100) == 1.0
    assert apply_scale.snap(M2560, 200) == 2.0
    assert apply_scale.snap(M2048, 200) == 2.0


def test_snap_rounded_requests_land_on_the_float32_member():
    # The regression this design exists for: 133/166 must map to mutter's
    # exact doubles, because sending 1.33 or 1.66 crashes the compositor.
    assert apply_scale.snap(M2560, 133) == 1.3333333730697632
    assert apply_scale.snap(M2560, 166) == 1.6666666269302368


@pytest.mark.parametrize("scales", [M1920, M2560, M2048, M1366])
def test_snap_always_returns_a_member_of_the_supported_set(scales):
    for pct in range(50, 400, 7):
        assert apply_scale.snap(scales, pct) in scales


def test_snap_out_of_range_requests_clamp_to_the_nearest_edge():
    assert apply_scale.snap(M1366, 200) == 1.0
    assert apply_scale.snap(M2048, 500) == 2.0


def test_snap_an_empty_set_yields_none():
    assert apply_scale.snap([], 150) is None


def test_current_mode_picks_the_is_current_mode_of_the_single_monitor():
    mode = apply_scale.current_mode(MONITORS)
    assert mode.mode_id == "2048x1152@60.000"
    assert (mode.width, mode.height) == (2048, 1152)
    assert mode.supported_scales == M2048
    assert mode.connector == "Virtual-1"


def test_current_mode_returns_none_when_no_mode_is_marked_current():
    monitors = [
        (
            MONITORS[0][0],
            [("2048x1152@60.000", 2048, 1152, 60.0, 1.0, M2048, {})],
            {},
        )
    ]
    assert apply_scale.current_mode(monitors) is None


@pytest.mark.parametrize("monitors", [[], MONITORS * 2])
def test_current_mode_rejects_anything_but_exactly_one_monitor(monitors):
    with pytest.raises(ValueError):
        apply_scale.current_mode(monitors)


def test_read_request_accepts_a_valid_percentage(tmp_path):
    request = tmp_path / "scale-request"
    request.write_text("150\n")
    assert apply_scale.read_request(str(request)) == 150


@pytest.mark.parametrize("bad", ["", "abc", "1.5", "0", "-150", "999"])
def test_read_request_rejects_garbage_and_out_of_range_values(bad, tmp_path):
    request = tmp_path / "scale-request"
    request.write_text(bad)
    with pytest.raises(ValueError):
        apply_scale.read_request(str(request))


def test_monitors_xml_renders_mutters_own_format():
    # Full-precision scale and a three-decimal rate, as mutter writes them.
    mode = apply_scale.Mode(
        "Virtual-1",
        "unknown",
        "unknown",
        "unknown",
        "2048x1152@60.000",
        2048,
        1152,
        60.0004882812,
        [1.0],
    )
    xml = apply_scale.monitors_xml(mode, 1.3333333730697632)
    assert "<scale>1.3333333730697632</scale>" in xml
    assert "<rate>60.000</rate>" in xml
    assert "<connector>Virtual-1</connector>" in xml
    assert "<width>2048</width>" in xml
    assert "<height>1152</height>" in xml


def test_monitors_xml_escapes_reserved_characters():
    # A virtio display reports plain names, but a monitor spec is EDID data --
    # reserved characters must not produce invalid XML mutter then ignores.
    mode = apply_scale.Mode(
        "Virtual-1",
        "A&B",
        "<Bad Product>",
        "unknown",
        "2048x1152@60.000",
        2048,
        1152,
        60.0,
        [1.0],
    )
    xml = apply_scale.monitors_xml(mode, 1.0)
    assert "<vendor>A&amp;B</vendor>" in xml
    assert "<product>&lt;Bad Product&gt;</product>" in xml
