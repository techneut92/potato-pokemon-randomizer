//! Writing data that may grow: free-space handling for GBA ROMs (finding unused space, freeing and taking space,
//! moving data and the copies of pointers to it).

use crate::RomError;
use crate::freespace::FreedSpace;
use crate::gba::GbaRom;

/// What unused space holds.
pub(super) const FREE_SPACE_BYTE: u8 = 0xFF;
/// Only runs of this many free bytes count as unused.
const CHUNK: usize = 0x100;
/// Kept at the start of a run, in case its first bytes belong to the data before.
const FRONT_MARGIN: usize = 0x10;
/// How far from a pointer [`pointer_copies`] looks for copies of it.
const POINTER_SEARCH_RADIUS: usize = 500;
/// GBA data is read in longs (4 bytes); everything moved is aligned to them.
const LONG: usize = 4;

/// The ROM together with its free space.
#[derive(Debug)]
pub(super) struct Space<'a> {
    pub(super) rom: &'a mut GbaRom,
    pub(super) free: &'a mut FreedSpace,
}

/// The first offset from `from` on where `CHUNK` free bytes start.
fn next_unused_chunk(bytes: &[u8], from: usize) -> Option<usize> {
    let mut run = 0;
    for (i, &b) in bytes.iter().enumerate().skip(from) {
        if b == FREE_SPACE_BYTE {
            run += 1;
            if run == CHUNK {
                return Some(i + 1 - CHUNK);
            }
        } else {
            run = 0;
        }
    }
    None
}

/// The unused space: from the entry's `FreeSpace` offset on, every 0x100-byte run of `0xFF`, less a
/// margin at the start of each separate run.
pub(super) fn unused_space(rom: &GbaRom, start: usize) -> Result<FreedSpace, RomError> {
    let mut free = FreedSpace::default();
    let mut offset = start;
    while let Some(found) = next_unused_chunk(rom.bytes(), offset) {
        if found > offset + CHUNK || found == start {
            free.free(found + FRONT_MARGIN, CHUNK - FRONT_MARGIN)?;
        } else {
            free.free(found, CHUNK)?;
        }
        offset = found + CHUNK;
    }
    Ok(free)
}

impl Space<'_> {
    /// Blanks the bytes and marks them free.
    pub(super) fn free(&mut self, offset: usize, length: usize) -> Result<(), RomError> {
        self.rom.write(offset, &vec![FREE_SPACE_BYTE; length])?;
        self.free.free(offset, length)
    }

    /// Takes room for `length` bytes at an offset divisible by 4.
    pub(super) fn find_and_unfree(&mut self, length: usize) -> Result<usize, RomError> {
        let padded = length + LONG;
        let found = loop {
            let found = self
                .free
                .find_and_unfree(padded)
                .ok_or_else(|| RomError::Malformed(format!("ROM full: no {padded} free bytes left")))?;
            // Something may have written there without taking the space; skip it then.
            if self.rom.slice(found, padded)?.iter().all(|&b| b == FREE_SPACE_BYTE) {
                break found;
            }
        };
        let shift = (LONG - found % LONG) % LONG;
        // Give back what the alignment leaves over at the end.
        self.free(found + padded - (LONG - shift), LONG - shift)?;
        Ok(found + shift)
    }

    /// Frees the string the pointer at `pointer_offset` points to (up to and
    /// including its `0xFF` terminator), writes `data` (terminator included) to free space and points
    /// `pointer_offset` and the `copies` there. A copy that pointed elsewhere is an error.
    pub(super) fn rewrite_string(
        &mut self,
        pointer_offset: usize,
        data: &[u8],
        copies: &[usize],
    ) -> Result<(), RomError> {
        let old = self.rom.pointer(pointer_offset)?;
        let old_length = self.rom.bytes()[old..]
            .iter()
            .position(|&b| b == FREE_SPACE_BYTE)
            .map(|i| i + 1)
            .ok_or_else(|| RomError::Malformed(format!("the string at {old:#x} has no end")))?;
        self.rewrite(pointer_offset, old_length, data, copies)
    }

    /// Frees the `old_length` bytes the pointer at `pointer_offset` points to,
    /// writes `data` to free space and points `pointer_offset` and the `copies` there. A copy that pointed
    /// elsewhere is an error.
    pub(super) fn rewrite(
        &mut self,
        pointer_offset: usize,
        old_length: usize,
        data: &[u8],
        copies: &[usize],
    ) -> Result<(), RomError> {
        let old = self.rom.pointer(pointer_offset)?;
        self.free(old, old_length)?;
        let new = self.find_and_unfree(data.len())?;
        self.rom.write_pointer(pointer_offset, new)?;
        self.rom.write(new, data)?;
        for &copy in copies {
            let target = self.rom.pointer(copy)?;
            if copy != pointer_offset && target != old {
                return Err(RomError::Malformed(format!(
                    "the pointer at {copy:#x} points to {target:#x}, not {old:#x}"
                )));
            }
            self.rom.write_pointer(copy, new)?;
        }
        Ok(())
    }
}

/// Other places within 500 bytes holding the same pointer (other scripts
/// showing the same text). The pointer itself is among them.
pub(super) fn pointer_copies(rom: &GbaRom, pointer_offset: usize) -> Result<Vec<usize>, RomError> {
    let needle = rom.slice(pointer_offset, 4)?.to_vec();
    let begin = pointer_offset.saturating_sub(POINTER_SEARCH_RADIUS);
    Ok(rom.search(begin, pointer_offset + POINTER_SEARCH_RADIUS, &needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_space_is_whole_chunks_less_a_front_margin() {
        let mut bytes = vec![0u8; 0x1000];
        // One long run from 0x100, one short run that does not count, one run of exactly a chunk.
        bytes[0x100..0x500].fill(0xFF);
        bytes[0x600..0x6F0].fill(0xFF);
        bytes[0x800..0x900].fill(0xFF);
        let free = unused_space(&GbaRom::new(bytes.clone()), 0x100).expect("valid");
        // Margin at the start (found == start), whole chunks after, margin again at the separate run.
        assert_eq!(free.len(), 0x400 - 0x10 + 0x100 - 0x10);
        // Starting inside a run: the first chunk found is the start, so it gets the margin.
        let free = unused_space(&GbaRom::new(bytes), 0x180).expect("valid");
        assert_eq!(free.len(), 0x300 - 0x10 + 0x100 - 0x10);
    }

    #[test]
    fn strings_move_to_aligned_free_space_with_their_copies() {
        let mut rom = GbaRom::new(vec![0u8; 0x400]);
        let mut free = FreedSpace::default();
        // Old string "AB" + terminator at 0x101, pointed to from 0x10 and 0x40.
        rom.write(0x101, &[0xBB, 0xBC, 0xFF]).expect("in range");
        rom.write_pointer(0x10, 0x101).expect("in range");
        rom.write_pointer(0x40, 0x101).expect("in range");
        rom.write(0x201, &[0xFF; 0x40]).expect("in range");
        free.free(0x201, 0x40).expect("free");
        let copies = pointer_copies(&rom, 0x10).expect("in range");
        assert_eq!(copies, [0x10, 0x40]);
        let mut space = Space {
            rom: &mut rom,
            free: &mut free,
        };
        space
            .rewrite_string(0x10, &[1, 2, 3, 4, 5, 0xFF], &copies)
            .expect("fits");
        // The freed old string (3 bytes) is too small; 0x201 aligned up is 0x204.
        assert_eq!(rom.pointer(0x10).expect("valid"), 0x204);
        assert_eq!(rom.pointer(0x40).expect("valid"), 0x204);
        assert_eq!(rom.bytes()[0x204..0x20A], [1, 2, 3, 4, 5, 0xFF]);
        assert_eq!(rom.bytes()[0x101..0x104], [0xFF; 3], "old string blanked");
    }

    #[test]
    fn a_copy_pointing_elsewhere_is_an_error() {
        let mut rom = GbaRom::new(vec![0u8; 0x400]);
        let mut free = FreedSpace::default();
        rom.write(0x100, &[0xBB, 0xFF]).expect("in range");
        rom.write_pointer(0x10, 0x100).expect("in range");
        rom.write_pointer(0x20, 0x300).expect("in range");
        rom.write(0x200, &[0xFF; 0x20]).expect("in range");
        free.free(0x200, 0x20).expect("free");
        let mut space = Space {
            rom: &mut rom,
            free: &mut free,
        };
        assert!(space.rewrite_string(0x10, &[1, 0xFF], &[0x10, 0x20]).is_err());
    }
}
