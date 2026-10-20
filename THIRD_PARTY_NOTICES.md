# Third-party notices

Original D1 Reimagined source and project-authored changes are separate from the external software and game inputs they may interact with. Existing file-level licence notices remain authoritative.

The `patches/rpcs3-*.patch` files contain scoped modifications and upstream source context from RPCS3. RPCS3 retains its contributors' copyright and GPL version 2 licence: https://github.com/RPCS3/rpcs3/blob/master/LICENSE. Preserve that licence and upstream notices in the resulting modified source. These increments require the appropriate external source baseline; they are not a bundled emulator or complete stock installation recipe.

The PS4 helpers retain their GPL-2.0-or-later file annotations. The `ps4/` subtree
now includes the supplied preservation project's original source, with its
existing GPL-2.0-or-later notices and `ps4/d1-server/ATTRIBUTION.md`. Its vendored
shadNet schema and client constants retain their upstream attribution. The
root GPL-2.0-only default does not replace these file/subtree notices.
shadPS4 itself and the separately installed LibOrbisPkg and ps4_unfself tools
are not bundled. A modified external LGPL reader must retain its upstream
license and corresponding modified source if redistributed; no such executable
is distributed here. Public-only synthetic fixtures and opt-in native golden
tests are documented in `ps4/README.md`.

Python dependencies are obtained separately through requirements.txt and retain their own licences. The consolidated data/ snapshot includes extracted Destiny executables and selected package assets, plus the historical oo2core_3_win64.dll binary and logs. Their original ownership and applicable terms remain separate from the project source licence. Firmware, private keys and account databases are not included. Destiny and related names and content belong to their respective rights holders. The source licence grants no rights to those materials.

Project-authored patch modifications are published as scoped changes on 12 October 2026. Their upstream context remains attributed to its original contributors; the source publication does not claim upstream authorship.
