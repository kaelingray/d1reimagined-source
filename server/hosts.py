"""Loopback addresses for Destiny service hostnames, shared by the sinkhole and listeners.

The PS3 sign-on client sends no SNI, so each hostname gets its own loopback IP and the
listener picks the certificate from the address that was dialled. Names were observed in
PS3 tests 003 and 006 (www1/www2/www8 on both domains) and in the Xbox 360 console
capture (www6.signon.gravityshavings.net); www1-www9 are reserved on each domain.
"""

STUN_HOST, STUN_IP = "dev-stun.demonware.net", "127.0.0.2"
STUN_HOSTS = {STUN_HOST: STUN_IP}
# 01.24 probes these regional and sign-on STUN names during startup. They must
# reach the stub on .2; resolving to .1 sends them back to RPCS3's own UDP port.
for region in ("us", "eu", "jp", "au"):
    STUN_HOSTS[f"destiny-stun.{region}.demonware.net"] = STUN_IP
for domain in ("deadorbit.net", "gravityshavings.net"):
    for n in range(1, 9):
        STUN_HOSTS[f"stun{n}.signon.{domain}"] = STUN_IP

SIGNON_HOSTS = {}
for n in range(1, 10):
    SIGNON_HOSTS[f"www{n}.signon.deadorbit.net"] = f"127.0.1.{n}"
    SIGNON_HOSTS[f"www{n}.signon.gravityshavings.net"] = f"127.0.2.{n}"

# Demonware HTTPS hosts named in the EBOOT (0x11f5614-0x11f5770); PS3 comparison 015 posted
# to panther-ps3-auth3.prod after SignOn. prod/cert/dev x lobby/auth/auth3 -> 127.0.3.x.
DEMONWARE_HOSTS = {}
for i, (env, svc) in enumerate((e, s) for e in ("prod", "cert", "dev") for s in ("lobby", "auth", "auth3")):
    DEMONWARE_HOSTS[f"panther-ps3-{svc}.{env}.demonware.net"] = f"127.0.3.{i + 1}"

HTTPS_HOSTS = {**SIGNON_HOSTS, **DEMONWARE_HOSTS}
ALL_OVERRIDES = {**STUN_HOSTS, **HTTPS_HOSTS}
