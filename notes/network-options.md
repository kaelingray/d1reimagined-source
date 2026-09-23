# Service path assessment

Reviewed: 2026-09-23. The goal is to identify a route beyond Destiny's canary screen while preserving the verified Xbox 360 capture. The current patched Xenia build imports Content 1–3 but still stops at canary.

## What the tests establish

- Test 010 observed 52,816 profile-state queries, 7,339 Ethernet-link checks, 734 title-address queries, and one undefined XHttp startup call. Pinned Xenia source reports a locally signed-in profile, an inactive Ethernet link, a static loopback title address without an online bit, and no XHttp implementation. See `notes/network-trace.md`.
- Test 011 changed only the Ethernet-link result to active. The same canary message appeared. That isolated change is insufficient; it does not prove which remaining check is first.
- No actual Destiny request to a game server was observed in these runs. The present Xenia service layer must be addressed before a private Destiny backend can be meaningfully tested.

## Existing code that may help

| Project | Verified in source | Gap for this goal |
| --- | --- | --- |
| [Nexia360 Xenia Canary netplay fork](https://github.com/Nexia360/xenia-canary), checked at `3fedf58218b8f0859b0c3d1d73e3938099ce8fb7` | Its `network_mode` can report Live sign-in and active/online XNet status. It implements DNS and some general netplay plumbing. | Its XHttp functions are still marked stubs: `NetDll_XHttpSendRequest` returns false and `NetDll_XHttpConnect` returns zero. No Destiny-specific implementation was found in the checkout. Our STFS content patch would need porting and retesting. |
| [Xenia Web Services](https://github.com/AdrianCassar/Xenia-WebServices), checked at `6abcc4397e0b6e0632307d9418ecae086cc95f48` | Provides a generic Live-like REST service and per-title configuration under `src/titles`. Its README says it does not represent the first-party Xbox API. | No `415608F8` Destiny title configuration or Bungie-specific backend was found in its source. It does not by itself supply Destiny's world, account, and activity services. |
| User-supplied Demonware examples in `notes/research-tools.md` | Show patterns for service protocol analysis in other games. | They are not Destiny servers and cannot be assumed wire-compatible with this Xbox 360 client. |

The [Xenia project roadmap](https://github.com/xenia-canary/xenia-canary/wiki/roadmap) explicitly separates simulated Live-like features from game-specific servers. The netplay fork could be a source of reusable XNet/profile code, but it is not yet a path to playable Destiny by configuration alone.

## Next evidence needed

1. Determine whether the owned Xbox 360 still signs in and reaches a character or activity screen. If so, capture a short, authorized run on the console network and record endpoints, timing, and failure/success sequence without publishing account identifiers or secrets.
2. Choose a testable milestone smaller than full gameplay, such as completing the title-screen service handshake against a local replacement. Record the exact service calls and responses that milestone needs.
3. Add only the necessary Xenia profile/XNet/XHttp support to a separate branch, or port the STFS patch onto a suitable netplay fork. Verify each change against the same baseline and keep any private service local.
4. Tackle the visible graphics corruption separately after the service path produces a later game state.

There is no evidence yet that a ready-made Destiny 1 Xbox 360 server exists in the examined repositories. That is a search result with limited scope, not proof that none exists.
