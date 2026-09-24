"""Create a throwaway account on a *local* RPCN server and point an RPCS3 install at it.

This never talks to the public RPCN service: the host must be a loopback address.
Credentials are random, stored only outside Git (in the RPCS3 config directory and a
private note next to the RPCN server), and exist only in the local server's database.

Wire format (RPCS3 rpcn_client.cpp / RPCN server.rs):
  header = u8 packet_type | u16le command | u32le total_size | u64le packet_id   (15 bytes)
  on connect the server sends a ServerInfo packet whose 4-byte payload is the protocol version.
  Create payload = npid\\0 password\\0 online_name\\0 avatar_url\\0 email\\0; reply payload[0] = ErrorType.
"""

import argparse
import ipaddress
import secrets
import socket
import ssl
import struct
import sys
from pathlib import Path

PACKET_REQUEST, PACKET_REPLY, PACKET_NOTIFICATION, PACKET_SERVERINFO = 0, 1, 2, 3
CMD_CREATE = 2
HEADER = struct.Struct("<BHIQ")
ERRORS = {
    0: "NoError", 3: "InvalidInput", 10: "CreationError", 11: "CreationExistingUsername",
    12: "CreationBannedEmailProvider", 13: "CreationExistingEmail",
}
DEFAULT_AVATAR = "https://rpcs3.net/cdn/netplay/DefaultAvatar.png"


def recv_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("server closed the connection")
        buf += chunk
    return buf


def recv_packet(sock):
    ptype, cmd, size, pid = HEADER.unpack(recv_exact(sock, HEADER.size))
    return ptype, cmd, pid, recv_exact(sock, size - HEADER.size)


def create_account(host, port, npid, password, email, expected_version):
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE  # local self-signed server, same as RPCS3's own client
    with socket.create_connection((host, port), timeout=10) as raw, ctx.wrap_socket(raw) as sock:
        ptype, _, _, data = recv_packet(sock)
        if ptype != PACKET_SERVERINFO or len(data) != 4:
            raise RuntimeError(f"expected ServerInfo, got packet type {ptype}")
        version = struct.unpack("<I", data)[0]
        print(f"RPCN protocol version: {version}")
        if expected_version is not None and version != expected_version:
            raise RuntimeError(f"server speaks protocol {version}, RPCS3 build expects {expected_version}")
        fields = [npid, password, npid, DEFAULT_AVATAR, email]
        payload = b"".join(f.encode("ascii") + b"\0" for f in fields)
        sock.sendall(HEADER.pack(PACKET_REQUEST, CMD_CREATE, HEADER.size + len(payload), 1) + payload)
        while True:
            ptype, cmd, pid, data = recv_packet(sock)
            if ptype == PACKET_REPLY and cmd == CMD_CREATE:
                code = data[0] if data else -1
                return code


def write_rpcs3_config(config_dir, host, port, npid, password):
    path = Path(config_dir) / "rpcn.yml"
    host_str = f"{host}:{port}"
    path.write_text(
        "Version: 2\n"
        f"Host: {host_str}\n"
        f"NPID: {npid}\n"
        f"Password: {password}\n"
        "Token: \"\"\n"
        f"Hosts: \"Official RPCN Server|np.rpcs3.net|||Local test RPCN|{host_str}|||\"\n"
        "Experimental IPv6 support: false\n",
        encoding="ascii",
    )
    return path


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=31313)
    ap.add_argument("--npid", default="d1localtest", help="3-16 chars: letters, digits, - or _")
    ap.add_argument("--expected-version", type=int, default=32, help="RPCN protocol of the RPCS3 build (32 for b5a174e9)")
    ap.add_argument("--rpcs3-config", help="RPCS3 config directory to receive rpcn.yml")
    ap.add_argument("--secret-file", required=True, help="where to store the generated password (keep outside Git)")
    args = ap.parse_args()

    if not ipaddress.ip_address(args.host).is_loopback:
        sys.exit("refusing: this helper only creates accounts on a loopback RPCN server")

    password = secrets.token_hex(32)
    email = f"{args.npid}@localhost.invalid"  # syntactically valid; local server has email validation disabled
    code = create_account(args.host, args.port, args.npid, password, email, args.expected_version)
    print(f"Create result: {code} ({ERRORS.get(code, 'unknown')})")
    if code != 0:
        sys.exit(1)

    Path(args.secret_file).write_text(f"npid={args.npid}\npassword={password}\nhost={args.host}:{args.port}\n", encoding="ascii")
    print(f"Credentials written to {args.secret_file}")
    if args.rpcs3_config:
        print(f"RPCS3 config written to {write_rpcs3_config(args.rpcs3_config, args.host, args.port, args.npid, password)}")


if __name__ == "__main__":
    main()
