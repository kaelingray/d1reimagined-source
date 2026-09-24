# Local Destiny service stand-ins (PS3 / RPCS3)

Loopback-only tools for replacing, one gate at a time, the online services Destiny's PS3 client contacts before gameplay. Every listener refuses non-loopback addresses, and the DNS sinkhole never forwards upstream, so a test run cannot reach live Bungie, Demonware or Sony hosts by name. Runtime data (CA keys, captured requests, logs) lives under `D:\Destiny360\SIGNON_LOCAL`, outside Git.

| File | Role |
| --- | --- |
| `dns_sinkhole.py` | UDP DNS on 127.0.0.1:53. Every A query gets 127.0.0.1; `--destiny-hosts` applies `hosts.py`. RPCS3's **DNS address** must be `127.0.0.1`. |
| `hosts.py` | Loopback address per service name: `dev-stun.demonware.net` → 127.0.0.2; `wwwN.signon.deadorbit.net` → 127.0.1.N; `wwwN.signon.gravityshavings.net` → 127.0.2.N; `panther-ps3-{lobby,auth,auth3}.{prod,cert,dev}.demonware.net` → 127.0.3.1–9. |
| `stun_stub.py` | Answers Demonware NAT/IP discovery on 127.0.0.2:3074 (types 30→31, 20→21). Clears error **marionberry**. |
| `signon_probe.py` | Sign-on listener on each host address, ports 443 and 80. Logs the ClientHello, completes the handshake (SSLv3 via `sslv3.py`, otherwise Python `ssl`), saves the HTTP request, replies `--status`; `--mode signon` answers `/SignOn` with a success message. |
| `sslv3.py` | Minimal SSL 3.0 server (RSA key exchange, RC4-SHA / 3DES-SHA). The PS3 client offers only SSLv3; current OpenSSL no longer implements it. |
| `local_ca.py` | Private test CA and per-host leaf certificates (SHA-1, PrintableString names, fixed 2014–2037 validity) made with Git's `openssl`. |
| `prepare_rpcs3_copy.py` | Copies a baseline RPCS3 folder to a new test folder, sets DNS to loopback, optional log levels, firmware root replacement (`--install-ca`) and the game CA patch (`--game-ca`). |
| `signon_proto.py` | Encoder/decoder for the `POST /SignOn` protobuf messages; the schema, read from the game's nanopb tables, is documented in the file. |
| `auth3_proto.py` | Synthetic PS3 auth3 response: task 19, code 700, echoed seed, packed client ticket and opaque server ticket. Enabled with `signon_probe.py --mode auth3`. |
| `session_listener.py` | Loopback TCP/UDP catch-all for the server address given in the SignOn reply (default 127.1.1.127:12850); logs and saves, never replies. |
| `game_ca_patch.py` | RPCS3 patch (`patches/imported_patch.yml` + `config/patch_config.yml`) that overwrites the root PEM embedded in Destiny's EBOOT (`0x011b7218`) with the local CA, in guest memory only. |

## Typical run

```bash
python server/prepare_rpcs3_copy.py D:/Destiny360/RPCS3_TEST003_LOCAL_RPCN D:/Destiny360/RPCS3_TESTnnn_NAME --log sys_net=Trace --log DnsHook=Trace --game-ca D:/Destiny360/SIGNON_LOCAL/ca_v4/ca.cert.pem
python -u server/dns_sinkhole.py --destiny-hosts --log D:/Destiny360/SIGNON_LOCAL/logs/testnnn-dns.log
python -u server/stun_stub.py
python -u server/signon_probe.py --mode signon --destiny-hosts
python -u server/session_listener.py
```

The local RPCN server (`D:\Destiny360\RPCN_LOCAL\rpcn.exe`) must also be running for PSN sign-in. Then boot `PS3_GAME\USRDIR\EBOOT.BIN` from the new RPCS3 folder and press X/A at the title screen.

## Current gate (2026-09-24)

With DNS, STUN and RPCN local, the game CA patch applied and `signon_probe.py --mode auth3`, Destiny completes SignOn and accepts the synthetic auth3 reply (comparison 016). It connects to `panther-ps3-lobby.prod.demonware.net` at `127.0.3.1:3074` and sends a 28-byte protocol negotiation message. A silent lobby listener produces **cattle** after about 10 seconds. Character selection is not implemented. See `notes/ps3-route.md`.

For this test, also run `python server/session_listener.py --bind 127.0.3.1 --port 3074 --capture-dir D:/Destiny360/SIGNON_LOCAL/captures/lobby`. Auth session keys and tickets are saved alongside each private auth request, for the next lobby implementation. Run wire-contract tests with `python -m unittest discover -s server -p test_auth3_proto.py -v`.
