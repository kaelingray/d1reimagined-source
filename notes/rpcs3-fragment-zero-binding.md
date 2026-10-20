# 01.24 fragment source: explicit zero-local binding

JOIN126 used the same starter inventory, renderer/cache and runtime configuration as
JOIN125, adding only gated source-binding diagnostics. The Release build completed and
the deployed binary matched the build SHA-256. The source change is preserved in
`patches/rpcs3-fragment-binding-probe.patch`; it uses the existing `d1fp.on` marker and
line budget and leaves shader/draw behavior unchanged.

Two short diagnostic windows yielded 798 binding records. The three captured instances
of the known malformed floating-point TXD program all had:

- fragment offset `0`, location `0` (local VRAM), resolved address `0xc0000000`;
- first bound floats `(-1, -1, 0, 1)`, followed by other vertex-position values;
- parser start `0x1f1c20`, code length `0x20`, literal length `0x10`;
- unrelated parameter floats at the selected start, including a changing fourth float.

The preliminary parser treats the early opcode-zero vertex words as NOPs, then walks
roughly two megabytes before floating-point data happens to resemble an ending TXD.
Changing parameter data generates changing program keys. The cinematic window also
contained zero-local bindings with a different distant parser start (`0x1f6210`).
Normal captured fragment programs started at their actual bound address.

A read-only, partial guest-memory scan found command buffers explicitly emitting:

```
NV4097_SET_COLOR_MASK                0
NV4097_SET_COLOR_MASK_MRT            0
NV4097_SET_SHADER_PROGRAM            1   # offset 0, local location
NV4097_SET_VERTEX_ATTRIB_OUTPUT_MASK  0
NV4097_SET_SHADER_CONTROL            0x03000000
```

This establishes an explicit guest zero-local binding rather than just an uninitialized
host field. Disabled color writes suggest a depth/stencil/occlusion path, but the capture
does not establish all alpha/coverage state or original hardware behavior. Draws must
be preserved when investigating a depth-only implementation; blindly skipping the
malformed draw would discard potentially required depth/stencil/occlusion work.

Historical RPCS3 commit `05ffb50037facbae55db104cc94294cb75351b63` handles the different
raw shader-program value `0` as invalid. Destiny's raw value here is `1`, which encodes
local address zero. That old commit therefore does not supply a fix for this case.

No first-16-byte before/after mismatch appeared in these 798 samples. They are bounded,
non-atomic reads and do not rule out other races. Through native time 12:02, the world
run still logged 12,347 invalid-source programs and 31,475 additions, with no fatal
entry in that snapshot. Ordinary world appearance is not sustained renderer acceptance.
JOIN126 was stopped by exact run identity with the client before its services.

The [guarded depth-only candidate](rpcs3-fragment-depth-only.md) now captures full
fragment-affecting state and replaces only eligible zero-local draws with an END NOP.
The first controlled run eliminates the known malformed source in its log, but still
hits Vulkan device loss. Keep it disabled by default pending world/menu validation.
The leading-NOP-literal metadata correction remains useful but cannot repair a binding
whose original memory contains vertices rather than shader instructions.
