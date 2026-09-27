"""Locations of the private runtime and data: the git-ignored `local` folder in the repository
(game dump, RPCS3 build and config, stand-in schemas, captures, logs), or D1_LOCAL if set."""

import os
from pathlib import Path

LOCAL = Path(os.environ.get("D1_LOCAL") or Path(__file__).resolve().parents[1] / "local")
SIGNON = LOCAL / "SIGNON_LOCAL"
RPCS3 = LOCAL / "RPCS3_TEST048_SETUP"
CONTROL = RPCS3 / "d1"  # the diagnostic RPCS3 build's marker files and PPU watch list
