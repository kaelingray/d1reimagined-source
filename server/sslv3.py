"""Minimal SSLv3 server side (RFC 6101) for the PS3 sign-on client.

Destiny's PS3 HTTPS client offers only SSL 3.0 with RSA key exchange and 3DES/RC4/DES
suites, which current OpenSSL builds no longer implement. This module implements just
enough of the server side for local testing: one full handshake (no resumption, no
client certificates, no renegotiation) with RSA key exchange and either
RSA_WITH_RC4_128_SHA (0x0005) or RSA_WITH_3DES_EDE_CBC_SHA (0x000A).

SSLv3 is broken cryptography; this exists only to talk to an old client over loopback.
"""

import hashlib
import os
import struct
import time

from cryptography.hazmat.decrepit.ciphers.algorithms import ARC4, TripleDES
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives.ciphers import Cipher, modes

SSL3 = b"\x03\x00"
CT_CCS, CT_ALERT, CT_HANDSHAKE, CT_APPDATA = 20, 21, 22, 23
HS_CLIENT_HELLO, HS_SERVER_HELLO, HS_CERTIFICATE, HS_SERVER_HELLO_DONE = 1, 2, 11, 14
HS_CLIENT_KEY_EXCHANGE, HS_FINISHED = 16, 20
SUITES = {0x0005: "RSA_RC4_128_SHA", 0x000A: "RSA_3DES_EDE_CBC_SHA"}
ALERTS = {0: "close_notify", 10: "unexpected_message", 20: "bad_record_mac", 40: "handshake_failure",
          41: "no_certificate", 42: "bad_certificate", 43: "unsupported_certificate", 44: "certificate_revoked",
          45: "certificate_expired", 46: "certificate_unknown", 47: "illegal_parameter", 48: "unknown_ca"}
PAD1_SHA, PAD2_SHA = b"\x36" * 40, b"\x5c" * 40
PAD1_MD5, PAD2_MD5 = b"\x36" * 48, b"\x5c" * 48


class SSLv3Error(Exception):
    pass


class PeerAlert(SSLv3Error):
    def __init__(self, level, desc):
        super().__init__(f"client sent {'fatal' if level == 2 else 'warning'} alert "
                         f"{desc} ({ALERTS.get(desc, 'unknown')})")
        self.level, self.desc = level, desc


def _md5(*parts):
    return hashlib.md5(b"".join(parts)).digest()


def _sha(*parts):
    return hashlib.sha1(b"".join(parts)).digest()


def _ssl3_prf(secret, r1, r2, length):
    """SSLv3 key expansion: MD5(secret + SHA('A' + secret + r1 + r2)) + MD5(... 'BB' ...) + ..."""
    out, i = b"", 0
    while len(out) < length:
        label = bytes([ord("A") + i]) * (i + 1)
        out += _md5(secret, _sha(label, secret, r1, r2))
        i += 1
    return out[:length]


class _Direction:
    """Record protection state for one direction."""

    def __init__(self, suite, mac_secret, key, iv, encrypt):
        self.mac_secret, self.seq, self.suite = mac_secret, 0, suite
        if suite == 0x0005:
            c = Cipher(ARC4(key), mode=None)
            self.block = 0
        else:
            c = Cipher(TripleDES(key), modes.CBC(iv))
            self.block = 8
        self.ctx = c.encryptor() if encrypt else c.decryptor()

    def mac(self, ctype, content):
        inner = _sha(self.mac_secret, PAD1_SHA, struct.pack(">QBH", self.seq, ctype, len(content)), content)
        return _sha(self.mac_secret, PAD2_SHA, inner)

    def protect(self, ctype, content):
        data = content + self.mac(ctype, content)
        self.seq += 1
        if self.block:
            pad = self.block - (len(data) + 1) % self.block
            data += bytes([pad]) * pad + bytes([pad])
        return self.ctx.update(data)

    def unprotect(self, ctype, fragment):
        data = self.ctx.update(fragment)
        if self.block:
            if not data or len(data) % self.block:
                raise SSLv3Error("block cipher record has bad length")
            pad = data[-1]
            if pad >= self.block:
                raise SSLv3Error("SSLv3 padding too long")
            data = data[:-(pad + 1)]
        content, mac = data[:-20], data[-20:]
        if mac != self.mac(ctype, content):
            raise SSLv3Error("record MAC mismatch")
        self.seq += 1
        return content


class SSLv3Server:
    """Server side of one SSLv3 connection over an already-accepted socket."""

    def __init__(self, sock, cert_chain_der, private_key, suite_preference=(0x0005, 0x000A)):
        self.sock, self.chain, self.key = sock, cert_chain_der, private_key
        self.preference = suite_preference
        self.read_state = self.write_state = None
        self.rbuf, self.hbuf, self.appbuf = b"", b"", b""
        self.transcript = b""
        self.suite = None

    # --- record layer -------------------------------------------------------
    def _recv_exact(self, n):
        while len(self.rbuf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise SSLv3Error("connection closed by client")
            self.rbuf += chunk
        out, self.rbuf = self.rbuf[:n], self.rbuf[n:]
        return out

    def _read_record(self):
        ctype, ver, length = struct.unpack(">B2sH", self._recv_exact(5))
        fragment = self._recv_exact(length)
        if self.read_state:
            fragment = self.read_state.unprotect(ctype, fragment)
        if ctype == CT_ALERT and len(fragment) >= 2:
            raise PeerAlert(fragment[0], fragment[1])
        return ctype, fragment

    def _write_record(self, ctype, content):
        if self.write_state:
            content = self.write_state.protect(ctype, content)
        self.sock.sendall(struct.pack(">B2sH", ctype, SSL3, len(content)) + content)

    def _read_handshake(self):
        while True:
            if len(self.hbuf) >= 4:
                length = int.from_bytes(self.hbuf[1:4], "big")
                if len(self.hbuf) >= 4 + length:
                    msg, self.hbuf = self.hbuf[:4 + length], self.hbuf[4 + length:]
                    return msg[0], msg[4:], msg
            ctype, frag = self._read_record()
            if ctype != CT_HANDSHAKE:
                raise SSLv3Error(f"expected handshake record, got content type {ctype}")
            self.hbuf += frag

    def _send_handshake(self, htype, body):
        msg = bytes([htype]) + len(body).to_bytes(3, "big") + body
        self.transcript += msg
        self._write_record(CT_HANDSHAKE, msg)

    def _finished_hash(self, sender, master):
        md5 = _md5(master, PAD2_MD5, _md5(self.transcript, sender, master, PAD1_MD5))
        sha = _sha(master, PAD2_SHA, _sha(self.transcript, sender, master, PAD1_SHA))
        return md5 + sha

    # --- handshake ----------------------------------------------------------
    def handshake(self):
        htype, body, raw = self._read_handshake()
        if htype != HS_CLIENT_HELLO:
            raise SSLv3Error(f"expected ClientHello, got handshake type {htype}")
        self.transcript += raw
        client_version = body[0:2]
        client_random = body[2:34]
        p = 34 + 1 + body[34]
        n = struct.unpack(">H", body[p:p + 2])[0]
        offered = {struct.unpack(">H", body[p + 2 + i:p + 4 + i])[0] for i in range(0, n, 2)}
        self.suite = next((s for s in self.preference if s in offered), None)
        if self.suite is None:
            self._write_record(CT_ALERT, bytes([2, 40]))
            raise SSLv3Error(f"no common suite; client offered {sorted(offered)}")

        server_random = struct.pack(">I", int(time.time())) + os.urandom(28)
        session_id = os.urandom(32)
        self._send_handshake(HS_SERVER_HELLO, SSL3 + server_random + bytes([32]) + session_id
                             + struct.pack(">HB", self.suite, 0))
        certs = b"".join(len(c).to_bytes(3, "big") + c for c in self.chain)
        self._send_handshake(HS_CERTIFICATE, len(certs).to_bytes(3, "big") + certs)
        self._send_handshake(HS_SERVER_HELLO_DONE, b"")

        htype, body, raw = self._read_handshake()
        if htype != HS_CLIENT_KEY_EXCHANGE:
            raise SSLv3Error(f"expected ClientKeyExchange, got handshake type {htype}")
        self.transcript += raw
        # SSLv3 sends the RSA ciphertext without the TLS 2-byte length prefix; accept both.
        key_bytes = (self.key.key_size + 7) // 8
        encrypted = body if len(body) == key_bytes else body[2:]
        premaster = self.key.decrypt(encrypted, padding.PKCS1v15())
        if len(premaster) != 48:
            raise SSLv3Error(f"premaster secret is {len(premaster)} bytes")
        if premaster[:2] != client_version:
            raise SSLv3Error("premaster version does not match ClientHello")

        master = b"".join(_md5(premaster, _sha(label, premaster, client_random, server_random))
                          for label in (b"A", b"BB", b"CCC"))
        key_len, iv_len = (16, 0) if self.suite == 0x0005 else (24, 8)
        block = _ssl3_prf(master, server_random, client_random, 2 * (20 + key_len + iv_len))
        cmac, smac = block[0:20], block[20:40]
        ckey, skey = block[40:40 + key_len], block[40 + key_len:40 + 2 * key_len]
        ivs = block[40 + 2 * key_len:]
        civ, siv = ivs[:iv_len], ivs[iv_len:]

        ctype, frag = self._read_record()
        if ctype != CT_CCS or frag != b"\x01":
            raise SSLv3Error(f"expected ChangeCipherSpec, got content type {ctype}")
        self.read_state = _Direction(self.suite, cmac, ckey, civ, encrypt=False)
        expected = self._finished_hash(b"CLNT", master)
        htype, body, raw = self._read_handshake()
        if htype != HS_FINISHED or body != expected:
            raise SSLv3Error("client Finished did not verify")
        self.transcript += raw

        self._write_record(CT_CCS, b"\x01")
        self.write_state = _Direction(self.suite, smac, skey, siv, encrypt=True)
        self._send_handshake(HS_FINISHED, self._finished_hash(b"SRVR", master))
        return SUITES[self.suite]

    # --- application data ---------------------------------------------------
    def recv(self, n=65536):
        while not self.appbuf:
            ctype, frag = self._read_record()
            if ctype == CT_APPDATA:
                self.appbuf += frag
            elif ctype != CT_HANDSHAKE:  # ignore stray handshake; anything else is unexpected
                raise SSLv3Error(f"unexpected content type {ctype}")
        out, self.appbuf = self.appbuf[:n], self.appbuf[n:]
        return out

    def sendall(self, data):
        for i in range(0, len(data), 16384):
            self._write_record(CT_APPDATA, data[i:i + 16384])

    def settimeout(self, t):
        self.sock.settimeout(t)

    def close_notify(self):
        try:
            self._write_record(CT_ALERT, bytes([1, 0]))
        except OSError:
            pass
