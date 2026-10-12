#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Archive Bungie's **Destiny 1** manifest - the name↔hash decoder ring - before it goes dark.

WHY THIS IS URGENT
------------------
D1's API is unsupported legacy. It has already broken once (April 2025; Bungie filed it as a bug and
were unsure of the cause). The endpoints are alive but **gated** - an earlier analysis concluded they
were 404/gone, which was wrong: `https://www.bungie.net/d1/Platform/Destiny/Manifest/` returns HTTP
200 with `ApiKeyMissingFromRequest` when unauthenticated. Alive-but-gated is not the same as dead,
and neither is the same as *permanent*.

WHY IT MATTERS TO THIS PROJECT
------------------------------
The manifest is thousands of **name ↔ hash pairs**. Bungie's content pipeline names things by hash,
and so does our game install. Without the mapping, a recovered definition hash is an opaque number.

WHAT THIS IS *NOT*
------------------
The public API is a **sibling** of the console service, not its wire specification. Treat its
definitions as a naming corpus and a data-model hint only. Where the final-state public data
disagrees with our owned 2016-era install, **the install wins** - the manifest here is version
`88857.20.11.05.1145-2` (Nov 2020), four years newer than patch 01.29.

ARCHIVE FORMAT - deliberately boring, deliberately standard
-----------------------------------------------------------
* **WARC 1.1** for the HTTP transactions (request, response, headers, timestamps). It is *the*
  web-archive standard, so this is readable by tools nobody here wrote.
* **BagIt (RFC 8493)** for the package inventory and SHA-256 fixity.

Both are chosen so the archive outlives this project's own tooling.

ORDERING - dependency order, not byte-volume order
--------------------------------------------------
The index is fetched and written first, then the language databases, then gear. **A hash-addressed
blob without its name↔hash mapping is practically unusable**, which makes the naming corpus
first-class payload rather than metadata.

CREDENTIAL HANDLING
-------------------
The API key is read from `~/.config/d1/bungie-api-key` (mode 600, outside the repo so it cannot be
committed) or `$BUNGIE_API_KEY`. It is **never** written into the archive: the WARC request records
have the `X-API-Key` header redacted. Only public, unauthenticated-scope endpoints are used - no
OAuth, no account data.

USAGE
-----
    ./fetch-d1-manifest.py --out manifest/            # everything
    ./fetch-d1-manifest.py --out manifest/ --index-only
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.request
from datetime import datetime, timezone

BASE = "https://www.bungie.net"
INDEX = f"{BASE}/d1/Platform/Destiny/Manifest/"
UA = "d1-preservation/1.0 (archival; contact via repo owner)"


def api_key():
    k = os.environ.get("BUNGIE_API_KEY")
    if k:
        return k.strip()
    p = os.path.expanduser("~/.config/d1/bungie-api-key")
    try:
        with open(p) as f:
            return f.read().strip()
    except OSError:
        print(f"no API key: set $BUNGIE_API_KEY or write {p}", file=sys.stderr)
        sys.exit(2)


def warc_record(kind, uri, headers_block, body, when):
    """One WARC 1.1 record. Hand-rolled: the format is simple and a dependency is not worth it."""
    rid = f"<urn:uuid:{hashlib.sha1((uri + kind + when).encode()).hexdigest()[:32]}>"
    payload = headers_block.encode() + b"\r\n" + body
    ctype = ("application/http;msgtype=request" if kind == "request"
             else "application/http;msgtype=response")
    hdr = (
        f"WARC/1.1\r\n"
        f"WARC-Type: {kind}\r\n"
        f"WARC-Target-URI: {uri}\r\n"
        f"WARC-Date: {when}\r\n"
        f"WARC-Record-ID: {rid}\r\n"
        f"WARC-Block-Digest: sha1:{hashlib.sha1(payload).hexdigest()}\r\n"
        f"Content-Type: {ctype}\r\n"
        f"Content-Length: {len(payload)}\r\n"
        f"\r\n"
    )
    return hdr.encode() + payload + b"\r\n\r\n"


def fetch(url, key, warc, note=""):
    """GET a URL, append request+response WARC records, return (bytes, sha256, status)."""
    when = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    req = urllib.request.Request(url, headers={"X-API-Key": key, "User-Agent": UA})
    # The key is REDACTED in the archived request record - the archive must be shareable.
    warc.write(warc_record(
        "request", url,
        f"GET {url} HTTP/1.1\r\nHost: www.bungie.net\r\n"
        f"X-API-Key: [REDACTED]\r\nUser-Agent: {UA}\r\n", b"", when))
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=180) as r:
        body = r.read()
        status = r.status
        hdrs = "".join(f"{k}: {v}\r\n" for k, v in r.headers.items())
    warc.write(warc_record(
        "response", url, f"HTTP/1.1 {status} OK\r\n{hdrs}", body, when))
    digest = hashlib.sha256(body).hexdigest()
    print(f"  {status}  {len(body):>11,} B  {digest[:16]}…  {note or url}"
          f"  ({time.time() - t0:.1f}s)")
    return body, digest, status, when


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="manifest", help="bag directory")
    ap.add_argument("--index-only", action="store_true")
    args = ap.parse_args()

    key = api_key()
    bag = os.path.abspath(args.out)
    data = os.path.join(bag, "data")
    os.makedirs(data, exist_ok=True)

    started = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    files = []  # (relpath, sha256, url, when, bytes)

    with open(os.path.join(bag, "d1-manifest.warc"), "wb") as warc:
        print("index:")
        body, digest, _st, when = fetch(INDEX, key, warc, "GetDestinyManifest")
        rel = "data/GetDestinyManifest.json"
        with open(os.path.join(bag, rel), "wb") as f:
            f.write(body)
        files.append((rel, digest, INDEX, when, len(body)))

        idx = json.loads(body)
        if idx.get("ErrorCode") != 1:
            print(f"API error: {idx.get('ErrorStatus')}", file=sys.stderr)
            return 1
        resp = idx["Response"]
        version = resp.get("version")
        print(f"\nmanifest version: {version}")

        targets = []
        if not args.index_only:
            # Naming corpus FIRST - the world content databases are the name<->hash mapping.
            for lang, path in sorted(resp.get("mobileWorldContentPaths", {}).items()):
                targets.append((f"world/{lang}", path))
            for g in resp.get("mobileGearAssetDataBases", []):
                if isinstance(g, dict) and g.get("path"):
                    targets.append((f"gear/v{g.get('version')}", g["path"]))
            if resp.get("mobileAssetContentPath"):
                targets.append(("asset", resp["mobileAssetContentPath"]))

        for name, path in targets:
            url = path if path.startswith("http") else BASE + path
            print(f"\n{name}:")
            try:
                body, digest, _st, when = fetch(url, key, warc, name)
            except Exception as e:  # noqa: BLE001 - a partial archive is still worth keeping
                print(f"  FAILED {name}: {e}", file=sys.stderr)
                continue
            rel = f"data/{name.replace('/', '_')}_{os.path.basename(path)}"
            with open(os.path.join(bag, rel), "wb") as f:
                f.write(body)
            files.append((rel, digest, url, when, len(body)))

    # ── BagIt (RFC 8493) ──
    with open(os.path.join(bag, "bagit.txt"), "w") as f:
        f.write("BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n")
    with open(os.path.join(bag, "manifest-sha256.txt"), "w") as f:
        for rel, digest, *_ in sorted(files):
            f.write(f"{digest}  {rel}\n")
    total = sum(n for *_, n in files)
    with open(os.path.join(bag, "bag-info.txt"), "w") as f:
        f.write(
            f"Source-Organization: Destiny 1 Preservation Project\n"
            f"Bagging-Date: {started[:10]}\n"
            f"External-Description: Bungie.net Destiny 1 manifest (unsupported legacy API), "
            f"version {version}. Naming corpus for hash-addressed content.\n"
            f"Payload-Oxum: {total}.{len(files)}\n")
    # Provenance beyond BagIt's remit: exact URLs and retrieval times, per file.
    with open(os.path.join(bag, "provenance.json"), "w") as f:
        json.dump({
            "manifest_version": version,
            "index_url": INDEX,
            "started_utc": started,
            "note": "Public API only, no OAuth scopes, no account data. The public API is a sibling "
                    "of the console service, NOT its wire specification; where it disagrees with the "
                    "owned 01.29 install, the install wins.",
            "files": [
                {"path": r, "sha256": d, "url": u, "retrieved_utc": w, "bytes": n}
                for r, d, u, w, n in sorted(files)
            ],
        }, f, indent=2)

    print(f"\nbag: {bag}")
    print(f"  {len(files)} files, {total:,} bytes, version {version}")
    print("  bagit.txt · manifest-sha256.txt · bag-info.txt · provenance.json · d1-manifest.warc")
    return 0


if __name__ == "__main__":
    sys.exit(main())
