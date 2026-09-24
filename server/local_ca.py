"""Private test CA for the local sign-on stub.

The CA key and every issued certificate live in a directory outside Git (default
D:\\Destiny360\\SIGNON_LOCAL\\ca_v4). Nothing here is trusted by Windows; the CA is only
ever installed into an isolated RPCS3 copy's dev_flash for local testing.

SHA-1 signatures are the default because every root in the PS3 firmware certificate
store is SHA-1 or MD5, and this 2014 client may not verify SHA-256 signatures. The
Python `cryptography` package refuses SHA-1 signing, so certificates are made with the
OpenSSL command-line tool (Git for Windows ships one).
"""

import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

DEFAULT_DIR = Path(r"D:\Destiny360\SIGNON_LOCAL\ca_v4")
# Same shape as the firmware roots Destiny loads (CA01-CA05): a three-entry subject in
# C, O, CN order, CA:TRUE without pathlen or key usage, plus subject and authority key
# identifiers, with every name encoded as PrintableString (REQ_CONFIG). The PS3 loader
# stopped at CA01 with OpenSSL's default UTF8String names (PS3 Tests 007-009). The names
# are our own; nothing here copies a real CA's identity.
ORG = "d1xenia local testing"
CA_SUBJECT = f"/C=XX/O={ORG}/CN=d1xenia Local Test CA"
DIGESTS = ("sha1", "sha256")
REQ_CONFIG = "[req]\nstring_mask=nombstr\ndistinguished_name=dn\n[dn]\n"
# Fixed validity well inside the emulated clock's range and before 2038 (32-bit time_t).
# A notBefore of "now" was one of the remaining differences when the loader still
# rejected ca_v3 in PS3 Test 012.
NOT_BEFORE, CA_NOT_AFTER, LEAF_NOT_AFTER = "20140101000000Z", "20371201000000Z", "20371130000000Z"


def openssl():
    exe = shutil.which("openssl") or r"C:\Program Files\Git\mingw64\bin\openssl.exe"
    if not Path(exe).exists():
        raise FileNotFoundError("openssl not found; install Git for Windows or add openssl to PATH")
    return exe


def run(*args):
    # MSYS_NO_PATHCONV stops Git's openssl from rewriting "/CN=..." subjects as paths.
    env = dict(os.environ, MSYS_NO_PATHCONV="1", MSYS2_ARG_CONV_EXCL="*")
    subprocess.run([openssl(), *map(str, args)], check=True, capture_output=True, env=env)


def to_lf(path):
    # Git's openssl writes PEM with CRLF on Windows; the firmware roots use LF. (LF alone
    # did not fix the PS3 loader in Test 009, but it keeps our files in the same form.)
    path = Path(path)
    path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n"))


def ca_paths(ca_dir):
    ca_dir = Path(ca_dir)
    return ca_dir / "ca.key.pem", ca_dir / "ca.cert.pem"


def load_or_create_ca(ca_dir=DEFAULT_DIR, digest="sha1"):
    key_path, cert_path = ca_paths(ca_dir)
    if key_path.exists() and cert_path.exists():
        return key_path, cert_path
    Path(ca_dir).mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        ext = Path(tmp) / "ca.ext"
        ext.write_text(REQ_CONFIG + "[v3_ca]\nsubjectKeyIdentifier=hash\n"
                       "authorityKeyIdentifier=keyid:always,issuer:always\nbasicConstraints=CA:TRUE\n")
        run("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key_path, "-out", cert_path,
            f"-{digest}", "-not_before", NOT_BEFORE, "-not_after", CA_NOT_AFTER, "-set_serial", "1",
            "-subj", CA_SUBJECT,
            "-config", ext, "-extensions", "v3_ca")
    to_lf(cert_path)
    to_lf(key_path)
    run("x509", "-in", cert_path, "-outform", "DER", "-out", Path(ca_dir) / "ca.cert.der")
    return key_path, cert_path


def issue_leaf(hostname, ca_dir=DEFAULT_DIR, digest="sha1"):
    """Return (chain_path, key_path) for hostname, issuing it on first use."""
    ca_key, ca_cert = load_or_create_ca(ca_dir, digest)
    leaf_dir = Path(ca_dir) / "leaf"
    chain_path, key_path = leaf_dir / f"{hostname}.{digest}.chain.pem", leaf_dir / f"{hostname}.{digest}.key.pem"
    if chain_path.exists() and key_path.exists():
        return chain_path, key_path
    leaf_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        ext = tmp / "leaf.ext"
        ext.write_text(f"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\n"
                       f"extendedKeyUsage=serverAuth\nsubjectAltName=DNS:{hostname}\n")
        csr, cert, req_cfg = tmp / "leaf.csr", tmp / "leaf.pem", tmp / "req.cnf"
        req_cfg.write_text(REQ_CONFIG)
        run("req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", key_path, "-out", csr,
            "-subj", f"/C=XX/O={ORG}/CN={hostname}", "-config", req_cfg)
        run("x509", "-req", "-in", csr, "-CA", ca_cert, "-CAkey", ca_key, "-set_serial",
            str(int.from_bytes(os.urandom(8), "big")), "-not_before", NOT_BEFORE, "-not_after", LEAF_NOT_AFTER,
            f"-{digest}", "-extfile", ext, "-out", cert)
        # Leaf then CA, so the client can build the path even with only the root installed.
        chain_path.write_bytes((cert.read_bytes() + ca_cert.read_bytes()).replace(b"\r\n", b"\n"))
        to_lf(key_path)
    return chain_path, key_path


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dir", type=Path, default=DEFAULT_DIR)
    ap.add_argument("--digest", choices=DIGESTS, default="sha1")
    ap.add_argument("hostnames", nargs="*", help="leaf certificates to issue now")
    args = ap.parse_args()
    print("CA:", load_or_create_ca(args.dir, args.digest)[1])
    for host in args.hostnames:
        print("leaf:", issue_leaf(host, args.dir, args.digest)[0])


if __name__ == "__main__":
    main()
