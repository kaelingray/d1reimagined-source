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

An optional second patch (`unsigned_manifest=True`) clears the byte at 0x0179d5f4, which
only 0xb6f3cc reads. While it is 1, the content manifest download (0xb6f338) must begin
with a 0x108-byte block (u32le 1, u32le 0, 256-byte RSA signature) that verifies
(0xb23b34, libtomcrypt) against a runtime key table at 0x01871c70; otherwise the result
is "signature failure". Cleared, the whole body is decoded as the manifest message.
"""

from pathlib import Path

PPU_HASH = "PPU-10d41d88e5ce442299c2ba4cb681f0b307f33527"
SERIAL = "BLES01857"
TITLE = "Destiny"
EMBEDDED_CA_ADDR = 0x011B7218
EMBEDDED_CA_LEN = 2390  # without the NUL terminator
DESCRIPTION = "d1xenia: local sign-on CA replaces embedded roots"
MANIFEST_FLAG_ADDR = 0x0179D5F4
MANIFEST_DESCRIPTION = "d1xenia: accept unsigned content manifest"


def yaml_quote(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def patch_entry(description, notes, lines):
    return (
        f"  {yaml_quote(description)}:\n"
        "    Games:\n"
        f"      {yaml_quote(TITLE)}:\n"
        f"        {SERIAL}: [ All ]\n"
        "    Author: \"d1xenia\"\n"
        f"    Notes: {yaml_quote(notes)}\n"
        "    Patch Version: 1.0\n"
        "    Patch:\n"
        + "".join(f"      - {line}\n" for line in lines)
    )


def patch_yaml(pem_text, unsigned_manifest=False):
    pem_text = pem_text.replace("\r\n", "\n")
    if len(pem_text.encode("ascii")) > EMBEDDED_CA_LEN:
        raise ValueError(f"CA PEM is {len(pem_text)} bytes; the embedded slot holds {EMBEDDED_CA_LEN}")
    end = EMBEDDED_CA_ADDR + len(pem_text)
    text = "Version: 1.2\n\n" + f"{PPU_HASH}:\n" + patch_entry(
        DESCRIPTION, "Loopback testing only. Trusts the private d1xenia CA instead of the game's own roots.",
        [f"[ utf8, 0x{EMBEDDED_CA_ADDR:08x}, {yaml_quote(pem_text)} ]", f"[ byte, 0x{end:08x}, 0x00 ]"])
    if unsigned_manifest:
        text += patch_entry(
            MANIFEST_DESCRIPTION, "Loopback testing only. Skips the content manifest signature check.",
            [f"[ byte, 0x{MANIFEST_FLAG_ADDR:08x}, 0x00 ]"])
    return text


def config_yaml(unsigned_manifest=False):
    names = [DESCRIPTION] + ([MANIFEST_DESCRIPTION] if unsigned_manifest else [])
    return f"{PPU_HASH}:\n" + "".join(
        f"  {yaml_quote(name)}:\n"
        f"    {yaml_quote(TITLE)}:\n"
        f"      {SERIAL}:\n"
        "        All:\n"
        "          Enabled: true\n" for name in names)


def install(rpcs3_dir: Path, ca_pem: Path, unsigned_manifest=False, replace=False):
    """Write the patch and enable it in an RPCS3 folder.

    Existing files are replaced only with replace=True, and only if this module wrote them.
    """
    patch_file = rpcs3_dir / "patches" / "imported_patch.yml"
    config_file = rpcs3_dir / "config" / "patch_config.yml"
    for f in (patch_file, config_file):
        if f.exists() and not (replace and DESCRIPTION in f.read_text(encoding="utf-8")):
            raise FileExistsError(f"{f} already exists; merge by hand")
    pem = ca_pem.read_text(encoding="ascii")
    if pem.count("BEGIN CERTIFICATE") != 1:
        raise ValueError("--game-ca must hold exactly one PEM certificate")
    patch_file.parent.mkdir(parents=True, exist_ok=True)
    patch_file.write_text(patch_yaml(pem, unsigned_manifest), encoding="utf-8", newline="\n")
    config_file.write_text(config_yaml(unsigned_manifest), encoding="utf-8", newline="\n")
    return patch_file, config_file
