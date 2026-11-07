//! Binary formats of Nintendo handheld ROMs.
//!
//! This crate knows file layouts (headers, containers, codecs) but nothing about Pokémon. Everything here
//! parses untrusted input, so it returns errors instead of panicking (ADR 0006).

pub mod base64;
pub mod blz;
mod checksum;
pub mod ctr;
mod error;
pub mod gb;
pub mod gba;
mod io;
pub mod lz;
pub mod lz10;
pub mod lz11;
pub mod lz3;
pub mod narc;
pub mod nds;

pub use checksum::{crc16, crc32, crc32_of};
pub use error::FormatError;
pub use io::{RomSource, ascii_field};
