"""RPCS3 patch that swaps Destiny's embedded sign-on root for the local test CA.

Destiny (BLES01857 01.00) does not rely on the firmware roots alone. At sign-on setup
(EBOOT 0x106880) it calls cellHttpsInit with two CA buffers:

  1. a PEM string built into the executable at 0x011b7218 (2,390 bytes): "Tiger Root
     Authority" (2013-2033), followed by GeoTrust Global CA;
  2. cellSslCertificateLoader(0x1f), i.e. firmware CA01-CA05.

Any failure there ends in the game's fatal path (Tests 007-013 broke step 2). Its SSL
callback (0x10816c) returns verifyErr with a mask of ignored flags cleared, so an
untrusted chain is refused.

This patch overwrites string 1 in guest memory with the local CA PEM (NUL-terminated),
leaving the firmware roots and the game's verification intact. RPCS3 applies it at load
time from <rpcs3>/patches/imported_patch.yml once enabled in config/patch_config.yml;
the game files on disk are unchanged. The addresses come from the EBOOT decrypted with
`rpcs3.exe --decrypt`; the hash is the "PPU executable hash" RPCS3 logs for that EBOOT.
"""

from pathlib import Path

PPU_HASH = "PPU-10d41d88e5ce442299c2ba4cb681f0b307f33527"
SERIAL = "BLES01857"
TITLE = "Destiny"
EMBEDDED_CA_ADDR = 0x011B7218
EMBEDDED_CA_LEN = 2390  # without the NUL terminator
DESCRIPTION = "d1xenia: local sign-on CA replaces embedded roots"


def yaml_quote(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def patch_yaml(pem_text):
    pem_text = pem_text.replace("\r\n", "\n")
    if len(pem_text.encode("ascii")) > EMBEDDED_CA_LEN:
        raise ValueError(f"CA PEM is {len(pem_text)} bytes; the embedded slot holds {EMBEDDED_CA_LEN}")
    end = EMBEDDED_CA_ADDR + len(pem_text)
    return (
        "Version: 1.2\n\n"
        f"{PPU_HASH}:\n"
        f"  {yaml_quote(DESCRIPTION)}:\n"
        "    Games:\n"
        f"      {yaml_quote(TITLE)}:\n"
        f"        {SERIAL}: [ All ]\n"
        "    Author: \"d1xenia\"\n"
        "    Notes: \"Loopback testing only. Trusts the private d1xenia CA instead of the game's own roots.\"\n"
        "    Patch Version: 1.0\n"
        "    Patch:\n"
        f"      - [ utf8, 0x{EMBEDDED_CA_ADDR:08x}, {yaml_quote(pem_text)} ]\n"
        f"      - [ byte, 0x{end:08x}, 0x00 ]\n"
    )


def config_yaml():
    return (
        f"{PPU_HASH}:\n"
        f"  {yaml_quote(DESCRIPTION)}:\n"
        f"    {yaml_quote(TITLE)}:\n"
        f"      {SERIAL}:\n"
        "        All:\n"
        "          Enabled: true\n"
    )


def install(rpcs3_dir: Path, ca_pem: Path):
    """Write the patch and enable it in an RPCS3 folder. Refuses to overwrite existing files."""
    patch_file = rpcs3_dir / "patches" / "imported_patch.yml"
    config_file = rpcs3_dir / "config" / "patch_config.yml"
    for f in (patch_file, config_file):
        if f.exists():
            raise FileExistsError(f"{f} already exists; merge by hand")
    pem = ca_pem.read_text(encoding="ascii")
    if pem.count("BEGIN CERTIFICATE") != 1:
        raise ValueError("--game-ca must hold exactly one PEM certificate")
    patch_file.parent.mkdir(parents=True, exist_ok=True)
    patch_file.write_text(patch_yaml(pem), encoding="utf-8", newline="\n")
    config_file.write_text(config_yaml(), encoding="utf-8", newline="\n")
    return patch_file, config_file
