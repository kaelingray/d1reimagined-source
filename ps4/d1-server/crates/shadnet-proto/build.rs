// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-FileCopyrightText: Copyright 2019-2026 rpcsn Project
// SPDX-FileCopyrightText: Copyright 2026 shadNet Project / shadPS4 Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Derived from shadPS4 (GPL-2.0): protocol constants, packet framing and enum
// discriminants follow src/shadnet/client.{h,cpp}. See ATTRIBUTION.md.
fn main() {
    let proto = "../../proto/shadnet.proto";
    println!("cargo:rerun-if-changed={proto}");
    prost_build::compile_protos(&[proto], &["../../proto"]).expect("failed to compile shadnet.proto");
}
