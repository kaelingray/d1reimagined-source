# Destiny title-screen network trace

Date: 2026-09-23. Test 010 used the final patched Xenia build from Test 009, with `log_level=3` and high-frequency kernel-call logging enabled in a separate `D:\Destiny360\XENIA_TEST010_NETWORK_TRACE` folder. The user pressed A, observed the same canary message, and closed Xenia. The full 182 MB log remains private at `D:\Destiny360\LOGS\Test010-network-trace.xenia.log`; this note contains no profile identifiers or request contents.

Calls observed in the run:

| Guest API | Calls | Behavior in pinned Xenia source |
| --- | ---: | --- |
| `XamUserGetSigninState` | 52,816 | Returns `SignedInLocally` for the signed-in profile. |
| `XamUserGetSigninInfo` | 5,868 | Reports the same local sign-in state; other user slots can return no user. |
| `NetDll_XNetGetEthernetLinkStatus` | 7,339 | Stub always returns `0`, meaning no active Ethernet link bit. |
| `NetDll_XNetGetTitleXnAddr` | 734 | Stub returns a loopback address with static-address status, without the online bit. |
| `NetDll_XNetStartup` | 1 | Returns success, but Xenia's source leaves its XNet initialization code commented out. |
| `NetDll_XHttpStartup` | 1 | Unimplemented external call; no subsequent `NetDll_XHttp*` call was observed. |

These counts cover the whole run, not a time-aligned segment immediately after pressing A. The logger records call arguments but not return values; the behavior column comes from pinned Xenia Canary source commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19`. The repeated zero link-status result is a concrete emulator limitation and a possible trigger for canary. The local sign-in state and unimplemented HTTP API remain independent service blockers.

The verbose trace recorded no calls to `NetDll_connect`, `NetDll_send`, `NetDll_sendto`, `NetDll_XNetDnsLookup`, or `NetDll_XHttpSendRequest` during this run. The one `XHttpSendRequest` text occurrence is in Xenia's import listing, not a call. This supports the narrower conclusion that Destiny did not reach a guest-visible server request in Test 010; it does not prove which earlier check caused canary.

Test 011 used a controlled diagnostic. Its separate patch, `patches/xenia-ethernet-diagnostic.patch`, changes only `NetDll_XNetGetEthernetLinkStatus` when process environment variable `XENIA_TEST_ETHERNET_ACTIVE=1` is set. It returns the active-link bit and logs one warning. The diagnostic executable was isolated at `D:\Destiny360\XENIA_TEST011_ETHERNET_ACTIVE`; the source change was removed after the test, leaving the production content-import patch intact. The user pressed A and still saw the same canary message. The override was active in the log. Thus reporting an active cable alone is insufficient to progress past canary. The game still has an offline Xenia profile, a title address without the online bit, and unimplemented HTTP startup. Test log: `D:\Destiny360\LOGS\Test011-ethernet-active.xenia.log`; screenshot: `D:\Destiny360\LOGS\Test011-ethernet-active-canary.png`.

Test 012 used `patches/xenia-live-state-diagnostic.patch` in a separate build, activated by `XENIA_TEST_LIVE_STATE=1`. It changes only `UserProfile::signin_state()` to report `SignedInToLive`; it does not authenticate a user or implement a service. The user pressed A and reported the same canary message. The log still shows one undefined `NetDll_XHttpStartup` call. Therefore the reported sign-in state alone is insufficient. Log: `D:\Destiny360\LOGS\Test012-live-state.xenia.log`.

Test 013 ran on 2026-09-24 with the separate `patches/xenia-xhttp-startup-diagnostic.patch`. It implements only `NetDll_XHttpStartup` as a diagnostic return of 1 without opening a connection. The isolated executable was launched from `D:\Destiny360\XENIA_TEST013_XHTTP_STARTUP` against the working installed game, imported Content 1–3, and title update. The window showed `v0.0.23.2`; the log confirms the XEX patch applied. After pressing A, Destiny still showed **canary**. The log includes `Diagnostic: Destiny called XHttpStartup; returning initialized` once and no later `NetDll_XHttp*` call. Therefore a successful startup return alone does not pass Destiny's network gate. This diagnostic does not establish whether a fuller HTTP implementation, another network API, or an actual service response is required. The full log stays private at `D:\Destiny360\LOGS\Test013-xhttp-startup.xenia.log`. The source change was removed from the local Xenia checkout after building, leaving the production content-import patch intact.
