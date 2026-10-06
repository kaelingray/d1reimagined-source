# Recoverable native recording fragments — 2026-10-06

JOIN139's final active source could not be accepted after a host exception because
the recorder had not written its closing container metadata. The last accepted
segment ended 28.4 seconds before that exception. JOIN144 again hits the unresolved
GPU write fault before its mission handoff; durable failure footage remains useful
while the renderer and protocol investigation continues.

The scoped [native patch](../patches/rpcs3-recording-fragments.patch) changes MOV and
MP4 recording headers to initial metadata plus fragments at keyframes or one-second
duration boundaries. The actual native recorder selects the **MOV muxer** for its
MPEG-4 Part 2/AAC streams, despite the `.mp4` output suffix. Both supported muxers
receive the options. Other formats keep their defaults. Dictionary allocation and
header failures propagate through the existing error path, and options are freed.

[FFmpeg's documentation](https://ffmpeg.org/ffmpeg-formats.html#Fragmentation)
explains interrupted-write recovery and the compatibility tradeoff of fragmented
containers. The [7.1 muxer source](https://github.com/FFmpeg/FFmpeg/blob/n7.1/libavformat/movenc.c)
also exposes the initial-metadata flag used by this patch. The local validation CLI
is 7.1; the rebuilt native executable links libavformat 62.12. Native verification
therefore remains a separate check.

An abrupt exit can still leave the last fragment incomplete. The capture worker
scans only top-level box headers and skips payloads by their checked lengths. It
requires initial metadata and a complete fragment/data pair before recovering a
prefix. Only the **closed private buffer copy** is shortened; the original remains
intact. The recovered copy must then pass the existing strict full decode. Buffer
metadata records retained/original byte counts and discarded tail bytes, and those
details follow selected segments into the package metadata. Regular unfinished
recordings without initial metadata are not retroactively recoverable.

Import verification hashes the original and retained prefix under one closed-source
handle, then compares the prefix with the buffer copy. An index receipt is committed
only after verification and durable capture intent. Failed copies remain retryable
and cannot enter an event window. Delayed imports use the recording's validated end
time. Retried segments are selected once, preferring verified recovery metadata.

`scripts/rpcs3_fragmented_recording_audit.py` extracts the actual native header block
into **12 C++ checks**. All pass; the exact previous block fails seven, covering
missing fragment options and their allocation/cleanup paths. Optional FFmpeg tests
generate synthetic 30-fps MPEG-4/AAC media, kill only their own retained writer, and
exercise the production prefix scanner and strict decoding/remux. Both interrupted
MOV and MP4 recover **120 clean frames / 4.01 seconds** with unchanged originals.
The baseline recovers zero frames (`moov atom not found`). A normally closed
three-second recording decodes all 90 frames without errors.

All **58 capture tests** pass, including eight prefix boundary/format cases, recovery
and original retention, corrupt-copy retry, chunk-boundary hashes, delayed timestamps
and segment deduplication. The scoped patch applies to the exact saved baseline;
its result matches the compiled source after newline normalization. The incremental
native Release build succeeds and the deployed executable hash matches the build.

JOIN145 deliberately interrupts only its identity-verified emulator at **08:14:59 UTC**,
before the native recording trailer. The actual recorder uses two B-frames; the
synthetic fixture uses none. Its unfinished source is **1,835,028 bytes**, with a
**1,808,249-byte** recoverable prefix and **26,779-byte** unfinished tail. The prefix
strictly decodes **688 video frames / 23.83 seconds**, with audio and no errors. The
original's SHA-256 remains unchanged. There is no finished `mfra` trailer.

This integration test exposed an old whole-file hash check after prefix recovery,
premature index commitment, delayed import timestamps and duplicate retry segments.
The repairs above address those cases. The first 108.09-second package repeated the
recovered segment. Its unchanged local and NAS copies are preserved; the NAS copy
is renamed from `.ready` to `.superseded`, and the replacement references its clip ID.
The corrected worker automatically packages **84.25 seconds** from **three distinct
segments**, creates two source frames and delivers the package. All **seven files**
and **six payload sizes/SHA-256 hashes** are independently verified. There are now
**ten ready packages** on the configured 500 GB share. The pending queue is empty
and the normal worker is healthy and idle. No user capture or transfer is required.

The footage supplies rendered game/menu recording evidence, not mission acceptance.
The interruption is intentional, not a newly observed game crash. All owned test
processes close; original watches, XInput and byte-identical title configuration are
restored, with no active experimental controls. This repair does not establish a fix
for the GPU fault, character textures or mission progression.
