//! ROM detection and per-generation ROM handlers.
//!
//! The supported ROMs are described by the ROM entry files, embedded unchanged (ADR 0003).

pub mod ctr;
pub mod ctr_text;
mod detect;
mod ds_palette;
pub mod entries;
mod error;
mod formes;
mod freespace;
mod gb;
mod gba;
mod gen1;
mod gen2;
mod gen3;
mod gen4;
mod gen5;
mod gen6;
mod gen7;
mod graphics;
mod handler;
mod ips;
pub mod nds_text;
mod text;
mod textfmt;
mod type_order;

/// Synthetic ROMs without copyrighted data, for tests in other crates.
#[cfg(feature = "test-fixtures")]
pub mod testing {
    /// A synthetic Emerald (U): Treecko, Torchic and Mudkip as starters, the real table layout, no game code.
    #[must_use]
    pub fn emerald() -> Vec<u8> {
        crate::gen3::fixtures::emerald().rom
    }

    /// A synthetic `FireRed` (U) 1.0, laid out like [`emerald`].
    #[must_use]
    pub fn fire_red() -> Vec<u8> {
        crate::gen3::fixtures::fire_red().rom
    }
}

pub use detect::{Detection, Unsupported, detect};
pub use error::RomError;
pub use graphics::player_graphics_samples;
pub use handler::{GameHandler, open, open_bytes};
