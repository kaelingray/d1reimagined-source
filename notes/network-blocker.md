# Network blocker after content import

Test 009 used the updated game and all three imported content files. Pressing A still produced Destiny error **canary**. This is a user-visible result, not a diagnosis of the exact failing function. See `notes/test-log.md` and `D:\Destiny360\LOGS\Test009-full-content-canary.png`.

[Xenia's own FAQ](https://github.com/xenia-canary/xenia-canary/wiki/FAQ) says it does not connect to official Xbox Live, and the [Destiny compatibility report](https://github.com/xenia-project/game-compatibility/issues/269) marks Live sign-in as required. Those project sources agree with the observed blocker; they do not explain the exact code path for this Canary build.

In pinned Xenia Canary source commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19`:

- `src/xenia/kernel/xam/user_profile.h` defines online and local sign-in states, but `UserProfile::signin_state()` always returns `SignedInLocally` (offline).
- `src/xenia/kernel/xam/xam_user.cc` exposes that state through `XamUserGetSigninState`.
- `src/xenia/kernel/xam/xam_table.inc` lists the `NetDll_XHttp*` exports, but this checkout has no implementation of `NetDll_XHttpStartup`. Test 009 logged an undefined external call to it once. The other XHttp exports should be audited before treating any service test as meaningful.
- `src/xenia/kernel/xam/xam_net.cc` currently gives `NetDll_XNetGetTitleXnAddr` a loopback address and the `XNET_GET_XNADDR_STATIC` result. Its source comment calls this a stub.

These facts make a game-service path the next research area. A useful next diagnostic is a narrow trace of the game's sign-in, XNet, and XHttp calls and return values around the title-screen transition. Do not interpret a fabricated online sign-in flag as working connectivity. The current test did not show the game mounting or using Content 1–3 before it reached canary. The graphics corruption is a separate issue.

No account secrets or personal profile data should be placed in this repository or shared logs. Keep captures of original traffic private and redact identifiers before comparing protocols.
