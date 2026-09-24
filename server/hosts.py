"""Loopback addresses for Destiny service hostnames, shared by the sinkhole and listeners.

The PS3 sign-on client sends no SNI, so each hostname gets its own loopback IP and the
listener picks the certificate from the address that was dialled. Names were observed in
PS3 tests 003 and 006 (www1/www2/www8 on both domains) and in the Xbox 360 console
capture (www6.signon.gravityshavings.net); www1-www9 are reserved on each domain.
"""

STUN_HOST, STUN_IP = "dev-stun.demonware.net", "127.0.0.2"

SIGNON_HOSTS = {}
for n in range(1, 10):
    SIGNON_HOSTS[f"www{n}.signon.deadorbit.net"] = f"127.0.1.{n}"
    SIGNON_HOSTS[f"www{n}.signon.gravityshavings.net"] = f"127.0.2.{n}"

ALL_OVERRIDES = {STUN_HOST: STUN_IP, **SIGNON_HOSTS}
