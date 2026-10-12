// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Loadout / spawn framework - **data only**.
//!
//! ## What this crate is, and deliberately is not
//!
//! This crate describes *what a character should be wearing and holding*. It knows nothing about how
//! that becomes bytes on a wire. That separation is a project non-negotiable, not a style choice:
//! `CLAUDE.md` → **"One protocol, one implementer."** All BAP / investment / QueueZ encoding lives in
//! `d1-bap`, written by one session. Two crates that each know half of a wire format produce a
//! *silent* model mismatch that surfaces months later as mysterious behaviour, never as a compile
//! error. So this crate produces a `Loadout` and a catalogue; something else turns those into
//! messages.
//!
//! ## The item → wire representation is `[U]` and will change
//!
//! The main session is still deriving the inventory object schema (family 4 / type 3, capacity 666,
//! element size `0xB8` - see `historicaldocs/design/investment-profile-contract.md`). We do **not** know how an item
//! hash is carried in that structure, whether it is the manifest hash at all, or what else an
//! instanced item needs. Three consequences are baked into the types here:
//!
//! 1. Every persisted document carries [`SCHEMA_VERSION`] and refuses to load a *newer* one.
//! 2. Deserialisation is **tolerant**: unknown fields are ignored and missing fields default, so a
//!    file written by a later revision still loads as long as its version is understood.
//! 3. Nothing in this crate encodes, packs, byte-orders or hashes anything.
//!
//! ## Provenance - read this before trusting any item in here
//!
//! The catalogue is extracted from the Bungie D1 *world content* manifest preserved at
//! `manifest/data/`, version **`88857.20.11.05.1145-2`** - a **2020** manifest. The project targets
//! build **01.29** (Bungie 2.4.1, 18 Oct 2016; see `ERA-TARGETING.md`). Item hashes are largely
//! stable across D1's life, but this is **not** an era-matched source and the manifest carries no
//! "date added" field. [`era`] does what can honestly be done with what is there, and its default
//! answer is *unknown*. Reach tag for everything derived from it: `static-client-inference` - except
//! that it is not even the client, it is Bungie's public API. Nothing here has been confirmed
//! against the 01.29 client.

pub mod apply;
pub mod catalogue;
pub mod era;
pub mod manifest;
pub mod model;

pub use apply::{ApplyRequest, ApplyResponse, ApplyTarget, APPLY_CONTRACT_VERSION, APPLY_PATH};
pub use catalogue::{Catalogue, CatalogueEntry, CatalogueHeader, Query};
pub use era::{Era, EraVerdict};
pub use model::{
    Class, Damage, Equipped, Gender, ItemRef, Loadout, Race, Severity, Slot, Tier, ValidationIssue,
    CLASS_TYPE_ANY, LEVEL_CAP_01_29, LIGHT_CAP_01_29, SCHEMA_VERSION,
};
