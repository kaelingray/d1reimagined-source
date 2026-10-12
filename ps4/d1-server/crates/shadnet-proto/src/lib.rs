// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-FileCopyrightText: Copyright 2019-2026 rpcsn Project
// SPDX-FileCopyrightText: Copyright 2026 shadNet Project / shadPS4 Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Derived from shadPS4 (GPL-2.0): protocol constants, packet framing and enum
// discriminants follow src/shadnet/client.{h,cpp}. See ATTRIBUTION.md.
//! ShadNet protobuf message types, generated from the vendored `proto/shadnet.proto`.
//!
//! The schema is shadPS4's own (GPL-2.0-or-later, derived from the rpcsn project). We compile it
//! rather than hand-rolling encoders so our wire format can never silently drift from the client's.

//! Provenance and hash: `proto/PROVENANCE.md`.
include!(concat!(env!("OUT_DIR"), "/shadnet.rs"));
