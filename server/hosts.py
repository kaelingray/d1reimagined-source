"""Loopback addresses for Destiny service hostnames, shared by the sinkhole and listeners.

The PS3 sign-on client sends no SNI, so each hostname gets its own loopback IP and the
listener picks the certificate from the address that was dialled. Names were observed in
PS3 tests 003 and 006 (www1/www2/www8 on both domains); the historical reference
capture also supplied www6.signon.gravityshavings.net. www1-www9 remain reserved
on each domain for compatibility with the existing local service configuration.
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

# 01.24 walks all 20 of these names, and the STUN dispatch bounds-check accepts
# 0..0x13 (also 20), indexed by a counter that advances 1,2,3... as the client
# works through the list. Sharing one address may leave it unable to tell the
# hosts apart, so distinct_stun_hosts() hands out one loopback address per name
# in that order; the stub can bind the same range with --bind-count.
STUN_HOST_ORDER = (
    [f"destiny-stun.{region}.demonware.net" for region in ("us", "eu", "jp", "au")]
    + [f"stun{n}.signon.{domain}"
       for domain in ("deadorbit.net", "gravityshavings.net")
       for n in range(1, 9)]
)


def distinct_stun_hosts(first_octet=2, start=2):
    """{name: '127.0.2.<n>'} giving each 01.24 STUN name its own loopback address.

    The stub binds the same range with --bind-count, so the client sees 20
    different STUN servers instead of one address answering for all of them.
    """
    return {name: f"127.0.{first_octet}.{start + i}"
            for i, name in enumerate(STUN_HOST_ORDER)}

SIGNON_HOSTS = {}
for n in range(1, 10):
    SIGNON_HOSTS[f"www{n}.signon.deadorbit.net"] = f"127.0.1.{n}"
    SIGNON_HOSTS[f"www{n}.signon.gravityshavings.net"] = f"127.0.2.{n}"

# Demonware HTTPS hosts named in the EBOOT (0x11f5614-0x11f5770); PS3 comparison 015 posted
# to panther-ps3-auth3.prod after SignOn. prod/cert/dev x lobby/auth/auth3 -> 127.0.3.x.
DEMONWARE_HOSTS = {}
for i, (env, svc) in enumerate((e, s) for e in ("prod", "cert", "dev") for s in ("lobby", "auth", "auth3")):
    DEMONWARE_HOSTS[f"panther-ps3-{svc}.{env}.demonware.net"] = f"127.0.3.{i + 1}"

# The BAP connection object on 01.24 holds this upload host and the path '/ticket_drop'
# (EBOOT 0x012d6598 / 0x012d65c0; the path is at 0x011bdc01 in 01.00 too, so it is not new).
# These aliases use addresses already assigned to www9 SignOn hosts. The HTTPS
# listener must bind each address once, rather than opening a second socket for
# the alias. Mapping the name does not implement the upload endpoint. Unknown
# DNS names also stay local because the sinkhole never forwards upstream.
UPLOAD_HOSTS = {
    "dm-partnernet.upload.deadorbit.net": "127.0.1.9",
    "dm-partnernet.upload.gravityshavings.net": "127.0.2.9",
}

HTTPS_HOSTS = {**SIGNON_HOSTS, **DEMONWARE_HOSTS, **UPLOAD_HOSTS}
ALL_OVERRIDES = {**STUN_HOSTS, **HTTPS_HOSTS}
