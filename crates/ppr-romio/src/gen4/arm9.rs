//! Room in the ARM9 binary for patches and repointed data.
//!
//! The end of the ARM9 holds a part copied to ITCM, a part copied to DTCM, and pointers saying where they go; the
//! area after them is overwritten at run time. So the binary is not extended at its end: the part copied to ITCM
//! grows instead, everything after it moves up, and the SDK's copy pointers (found after `TCMCopyingPrefix`) and
//! the ITCM size are corrected. Code and data in the new room run from ITCM, so pointers to it point there.
//!
//! Of the room, the last `Arm9RepointExtensionSize` bytes are free for repointed data; the rest is for patches,
//! which bring their own offsets.

use super::files::{long, put_long, search};
use crate::RomError;
use crate::freespace::FreedSpace;

/// Where the ARM9 is loaded in RAM.
pub(super) const ARM9_RAM: usize = 0x0200_0000;
const ITCM_RAM: usize = 0x0100_0000;
const ITCM_END: usize = 0x01FF_FFFF;
const ITCM_LENGTH: usize = 0x8000;

/// What the extension left: where the copy pointers are and the room that is free.
#[derive(Debug, Clone, Default)]
pub(super) struct Arm9Room {
    /// Offset of the copy pointers; `None` when the ARM9 was not extended.
    tcm_pointers: Option<usize>,
    /// Space free for repointed data.
    pub(super) free: FreedSpace,
}

fn word_at(data: &[u8], at: usize) -> Result<usize, RomError> {
    Ok(long(data, at)? as usize)
}

fn to_u32(v: usize) -> Result<u32, RomError> {
    u32::try_from(v).map_err(|_| RomError::Malformed(format!("{v:#x} does not fit 32 bits")))
}

/// Extends `arm9` by `extend_by` bytes, of which the last `repoint_by` are free for data. `prefix` comes right
/// before the copy pointers.
pub(super) fn extend(
    arm9: &mut Vec<u8>,
    extend_by: usize,
    repoint_by: usize,
    prefix: &[u8],
) -> Result<Arm9Room, RomError> {
    if repoint_by > extend_by {
        return Err(RomError::Malformed(
            "more room to repoint into than the extension".into(),
        ));
    }
    let start = *search(arm9, prefix)
        .first()
        .ok_or_else(|| RomError::Malformed("no ARM9 copy pointers after TCMCopyingPrefix".into()))?;
    let pointers = start + prefix.len();
    let dest_pointers = word_at(arm9, pointers)?
        .checked_sub(ARM9_RAM)
        .ok_or_else(|| RomError::Malformed("ARM9 copy pointer below the ARM9".into()))?;
    let itcm_src = word_at(arm9, pointers + 8)?
        .checked_sub(ARM9_RAM)
        .ok_or_else(|| RomError::Malformed("ARM9 copy pointer below the ARM9".into()))?;
    let itcm_size_at = dest_pointers + 4;
    let itcm_size = word_at(arm9, itcm_size_at)?;
    if itcm_size + extend_by > ITCM_LENGTH {
        return Err(RomError::Malformed(
            "the part copied to ITCM can't grow past 32 KiB".into(),
        ));
    }
    let dtcm = itcm_src + itcm_size;
    if dtcm > arm9.len() {
        return Err(RomError::Malformed("the part copied to ITCM ends past the ARM9".into()));
    }
    let old_len = arm9.len();
    arm9.resize(old_len + extend_by, 0);
    put_long(arm9, pointers, to_u32(dest_pointers + extend_by + ARM9_RAM)?)?;
    let new_len = arm9.len();
    put_long(arm9, pointers + 4, to_u32(new_len + ARM9_RAM)?)?;
    put_long(arm9, itcm_size_at, to_u32(itcm_size + extend_by)?)?;
    arm9.copy_within(dtcm..old_len, dtcm + extend_by);
    let mut free = FreedSpace::default();
    if repoint_by > 0 {
        free.free(itcm_src + itcm_size + extend_by - repoint_by, repoint_by)?;
    }
    Ok(Arm9Room {
        tcm_pointers: Some(pointers),
        free,
    })
}

impl Arm9Room {
    fn pointers(&self) -> Result<usize, RomError> {
        self.tcm_pointers
            .ok_or_else(|| RomError::Malformed("the ARM9 was not extended".into()))
    }

    fn itcm_src(&self, arm9: &[u8]) -> Result<usize, RomError> {
        Ok(word_at(arm9, self.pointers()? + 8)?.saturating_sub(ARM9_RAM))
    }

    fn in_itcm_part(&self, arm9: &[u8], offset: usize) -> Result<bool, RomError> {
        let size_at = word_at(arm9, self.pointers()?)?.saturating_sub(ARM9_RAM) + 4;
        let size = word_at(arm9, size_at)?;
        let src = self.itcm_src(arm9)?;
        Ok(offset >= src && offset < src + size)
    }

    /// The ARM9 offset a pointer at `at` points to, in the ARM9 or (copied) in ITCM.
    pub(super) fn read_pointer(&self, arm9: &[u8], data: &[u8], at: usize) -> Result<usize, RomError> {
        let pointer = word_at(data, at)?;
        if pointer <= ITCM_END {
            Ok(pointer % ITCM_LENGTH + self.itcm_src(arm9)?)
        } else if pointer >= ARM9_RAM && pointer < ARM9_RAM + arm9.len() {
            Ok(pointer - ARM9_RAM)
        } else {
            Err(RomError::Malformed(format!(
                "{pointer:#x} points neither into the ARM9 nor into ITCM"
            )))
        }
    }

    /// Writes at `at` of `data` a pointer to ARM9 offset `target`, pointing into ITCM when it is copied there.
    pub(super) fn pointer_value(&self, arm9: &[u8], target: usize) -> Result<u32, RomError> {
        if self.in_itcm_part(arm9, target)? {
            to_u32(target - self.itcm_src(arm9)? + ITCM_RAM)
        } else if target < arm9.len() {
            to_u32(target + ARM9_RAM)
        } else {
            Err(RomError::Malformed(format!("{target:#x} is not in the ARM9")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen4::files::put;

    /// An ARM9 with the copy pointers after `PREFIX` at 0x10, an ITCM part at 0x40 of 0x20 bytes, DTCM after it,
    /// and the destination pointers at 0x70.
    fn sample() -> Vec<u8> {
        let mut arm9 = vec![0u8; 0x80];
        put(&mut arm9, 0x0C, b"PREF").expect("fits");
        put_long(&mut arm9, 0x10, to_u32(ARM9_RAM + 0x70).expect("fits")).expect("fits");
        put_long(&mut arm9, 0x14, to_u32(ARM9_RAM + 0x80).expect("fits")).expect("fits");
        put_long(&mut arm9, 0x18, to_u32(ARM9_RAM + 0x40).expect("fits")).expect("fits");
        put_long(&mut arm9, 0x74, 0x20).expect("fits");
        for (i, b) in arm9.iter_mut().enumerate().skip(0x40).take(0x30) {
            *b = u8::try_from(i).expect("small");
        }
        arm9
    }

    #[test]
    fn extending_moves_what_follows_the_itcm_part() {
        let mut arm9 = sample();
        let mut room = extend(&mut arm9, 0x10, 0x8, b"PREF").expect("extends");
        assert_eq!(arm9.len(), 0x90);
        assert_eq!(long(&arm9, 0x10).ok(), to_u32(ARM9_RAM + 0x80).ok());
        assert_eq!(long(&arm9, 0x14).ok(), to_u32(ARM9_RAM + 0x90).ok());
        // The ITCM size, now 0x30, moved up with the destination pointers.
        assert_eq!(long(&arm9, 0x84).ok(), Some(0x30));
        // DTCM moved up by 0x10.
        assert_eq!(arm9[0x70], 0x60);
        assert_eq!(room.free.find_and_unfree(8), Some(0x68));
        assert_eq!(room.pointer_value(&arm9, 0x68).ok(), to_u32(ITCM_RAM + 0x28).ok());
        assert_eq!(room.pointer_value(&arm9, 0x20).ok(), to_u32(ARM9_RAM + 0x20).ok());
        let mut data = vec![0u8; 4];
        put_long(&mut data, 0, to_u32(ITCM_RAM + 0x28).expect("fits")).expect("fits");
        assert_eq!(room.read_pointer(&arm9, &data, 0).ok(), Some(0x68));
    }

    #[test]
    fn refuses_to_grow_past_itcm() {
        let mut arm9 = sample();
        assert!(extend(&mut arm9, ITCM_LENGTH, 0, b"PREF").is_err());
        assert!(extend(&mut arm9, 4, 8, b"PREF").is_err());
    }
}
