//! Free space: the parts of a ROM that may be written to, as sorted, non-touching chunks.

use crate::RomError;

/// Free chunks as inclusive `(start, end)` offsets, sorted and never adjacent or overlapping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FreedSpace {
    chunks: Vec<(usize, usize)>,
}

impl FreedSpace {
    /// Marks `length` bytes from `start` as free, merging with neighbouring chunks. Freeing space that is already
    /// free is an error, as it points to a bug.
    pub(crate) fn free(&mut self, start: usize, length: usize) -> Result<(), RomError> {
        if length == 0 {
            return Err(RomError::Malformed("cannot free 0 bytes".into()));
        }
        let end = start + length - 1;
        // The first chunk that ends at or after the byte before `start`.
        let i = self.chunks.partition_point(|&(_, e)| e + 1 < start);
        if let Some(&(s, e)) = self.chunks.get(i)
            && s <= end
            && e >= start
        {
            return Err(RomError::Malformed(format!(
                "{start:#x}-{end:#x} overlaps space that is already free ({s:#x}-{e:#x})"
            )));
        }
        let joins_left = self.chunks.get(i).is_some_and(|&(_, e)| e + 1 == start);
        let right = if joins_left { i + 1 } else { i };
        let joins_right = self.chunks.get(right).is_some_and(|&(s, _)| s == end + 1);
        if let Some(&(s, e)) = self.chunks.get(right)
            && !joins_right
            && s <= end
        {
            return Err(RomError::Malformed(format!(
                "{start:#x}-{end:#x} overlaps space that is already free ({s:#x}-{e:#x})"
            )));
        }
        match (joins_left, joins_right) {
            (true, true) => {
                self.chunks[i].1 = self.chunks[right].1;
                self.chunks.remove(right);
            }
            (true, false) => self.chunks[i].1 = end,
            (false, true) => self.chunks[right].0 = start,
            (false, false) => self.chunks.insert(i, (start, end)),
        }
        Ok(())
    }

    /// Takes `length` bytes from the start of the first chunk that is big enough.
    pub(crate) fn find_and_unfree(&mut self, length: usize) -> Option<usize> {
        let i = self.chunks.iter().position(|&(s, e)| e - s + 1 >= length)?;
        let start = self.chunks[i].0;
        self.chunks[i].0 += length;
        if self.chunks[i].0 > self.chunks[i].1 {
            self.chunks.remove(i);
        }
        Some(start)
    }

    /// Like [`find_and_unfree`](Self::find_and_unfree), within `lo..=hi`: chunks count as if cut at both ends, so
    /// a chunk that runs over a bank border is free space in both banks.
    pub(crate) fn find_and_unfree_within(&mut self, lo: usize, hi: usize, length: usize) -> Option<usize> {
        if length == 0 {
            return None;
        }
        let i = self
            .chunks
            .iter()
            .position(|&(s, e)| s <= hi && e >= lo && e.min(hi) - s.max(lo) + 1 >= length)?;
        let (s, e) = self.chunks[i];
        let start = s.max(lo);
        let after = start + length;
        let mut rest = Vec::with_capacity(2);
        if s < start {
            rest.push((s, start - 1));
        }
        if after <= e {
            rest.push((after, e));
        }
        self.chunks.splice(i..=i, rest);
        Some(start)
    }

    /// Takes `length` bytes from `start` out of the free space, wherever they are free, e.g. where a patch with
    /// fixed offsets is going to write.
    pub(crate) fn reserve(&mut self, start: usize, length: usize) {
        if length == 0 {
            return;
        }
        let end = start + length - 1;
        let mut kept = Vec::with_capacity(self.chunks.len() + 1);
        for &(s, e) in &self.chunks {
            if e < start || s > end {
                kept.push((s, e));
                continue;
            }
            if s < start {
                kept.push((s, start - 1));
            }
            if e > end {
                kept.push((end + 1, e));
            }
        }
        self.chunks = kept;
    }

    /// Total free bytes.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.chunks.iter().map(|&(s, e)| e - s + 1).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(space: &FreedSpace) -> &[(usize, usize)] {
        &space.chunks
    }

    #[test]
    fn freeing_merges_neighbours() {
        let mut space = FreedSpace::default();
        space.free(10, 5).expect("free");
        space.free(30, 5).expect("free");
        space.free(0, 2).expect("free");
        assert_eq!(chunks(&space), [(0, 1), (10, 14), (30, 34)]);
        space.free(15, 15).expect("joins both");
        assert_eq!(chunks(&space), [(0, 1), (10, 34)]);
        space.free(2, 3).expect("joins left");
        space.free(8, 2).expect("joins right");
        assert_eq!(chunks(&space), [(0, 4), (8, 34)]);
        space.free(40, 1).expect("after all");
        assert_eq!(chunks(&space), [(0, 4), (8, 34), (40, 40)]);
        assert_eq!(space.len(), 33);
    }

    #[test]
    fn freeing_free_space_is_an_error() {
        let mut space = FreedSpace::default();
        space.free(10, 10).expect("free");
        for (start, len) in [(10, 1), (19, 5), (5, 6), (12, 2), (0, 40)] {
            assert!(space.free(start, len).is_err(), "{start} {len}");
        }
        assert!(space.free(0, 0).is_err());
        assert_eq!(chunks(&space), [(10, 19)]);
    }

    #[test]
    fn reserving_cuts_chunks() {
        let mut space = FreedSpace::default();
        space.free(0, 10).expect("free");
        space.free(20, 10).expect("free");
        space.free(40, 10).expect("free");
        space.reserve(5, 20);
        assert_eq!(chunks(&space), [(0, 4), (25, 29), (40, 49)]);
        space.reserve(42, 2);
        assert_eq!(chunks(&space), [(0, 4), (25, 29), (40, 41), (44, 49)]);
        space.reserve(30, 10);
        space.reserve(0, 0);
        assert_eq!(space.len(), 5 + 5 + 2 + 6);
    }

    #[test]
    fn takes_from_the_first_chunk_that_fits() {
        let mut space = FreedSpace::default();
        space.free(0, 4).expect("free");
        space.free(10, 10).expect("free");
        assert_eq!(space.find_and_unfree(6), Some(10));
        assert_eq!(chunks(&space), [(0, 3), (16, 19)]);
        assert_eq!(space.find_and_unfree(4), Some(0));
        assert_eq!(space.find_and_unfree(4), Some(16));
        assert_eq!(space.find_and_unfree(1), None);
    }

    #[test]
    fn a_chunk_over_a_border_is_free_on_both_sides() {
        let mut space = FreedSpace::default();
        space.free(0, 4).expect("free");
        space.free(10, 20).expect("free");
        // 10..=29 runs over a border at 20: 10 bytes on each side.
        assert_eq!(space.find_and_unfree_within(20, 39, 11), None);
        assert_eq!(space.find_and_unfree_within(20, 39, 6), Some(20));
        assert_eq!(chunks(&space), [(0, 3), (10, 19), (26, 29)]);
        assert_eq!(space.find_and_unfree_within(0, 19, 10), Some(10));
        assert_eq!(space.find_and_unfree_within(0, 19, 4), Some(0));
        assert_eq!(chunks(&space), [(26, 29)]);
        assert_eq!(space.find_and_unfree_within(0, 19, 1), None);
    }
}
