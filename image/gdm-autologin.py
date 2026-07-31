#!/usr/bin/env python3
"""Enable GDM autologin for USER by editing /etc/gdm/custom.conf in place.

Run by the first-boot provisioner (image/provision.sh) when the host asked for
autologin. Editing with configparser rather than overwriting the file keeps the
[daemon] section and the others Fedora and Bluefin ship; only the two autologin
keys are touched.
"""

import configparser
import os
import sys

PATH = "/etc/gdm/custom.conf"


class CaseSensitiveConfig(configparser.ConfigParser):
    """ConfigParser that keeps option-name case -- GDM keys are case-sensitive."""

    def optionxform(self, optionstr: str) -> str:
        return optionstr


def enable_autologin(user):
    cfg = CaseSensitiveConfig()
    if os.path.exists(PATH):
        cfg.read(PATH)
    if not cfg.has_section("daemon"):
        cfg.add_section("daemon")
    cfg["daemon"]["AutomaticLoginEnable"] = "true"
    cfg["daemon"]["AutomaticLogin"] = user
    os.makedirs(os.path.dirname(PATH), exist_ok=True)
    with open(PATH, "w") as f:
        cfg.write(f)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: bluefin-vm-gdm-autologin USER")
    enable_autologin(sys.argv[1])
