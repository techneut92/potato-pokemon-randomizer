use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};

use crate::FormatError;

/// Random-access input. Header readers only touch the bytes they need, so multi-GB images are never read whole.
pub trait RomSource: Read + Seek {}

impl<T: Read + Seek + ?Sized> RomSource for T {}

/// Reads exactly `N` bytes at `offset`.
pub(crate) fn read_at<const N: usize>(
    src: &mut (impl RomSource + ?Sized),
    offset: u64,
) -> Result<[u8; N], FormatError> {
    let mut buf = [0u8; N];
    src.seek(SeekFrom::Start(offset))?;
    src.read_exact(&mut buf).map_err(|e| match e.kind() {
        ErrorKind::UnexpectedEof => FormatError::Truncated { offset },
        _ => FormatError::Io(e),
    })?;
    Ok(buf)
}

/// Reads exactly `len` bytes at `offset`.
pub(crate) fn read_vec_at(
    src: &mut (impl RomSource + ?Sized),
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, FormatError> {
    let mut buf = vec![0u8; len];
    read_into(src, offset, &mut buf)?;
    Ok(buf)
}

/// Fills `buf` with the bytes at `offset`.
pub(crate) fn read_into(src: &mut (impl RomSource + ?Sized), offset: u64, buf: &mut [u8]) -> Result<(), FormatError> {
    src.seek(SeekFrom::Start(offset))?;
    src.read_exact(buf).map_err(|e| match e.kind() {
        ErrorKind::UnexpectedEof => FormatError::Truncated { offset },
        _ => FormatError::Io(e),
    })
}

/// Copies `len` bytes at `offset` of `src` to `out`, a chunk at a time.
pub(crate) fn copy_range(
    src: &mut (impl RomSource + ?Sized),
    offset: u64,
    len: u64,
    out: &mut (impl Write + ?Sized),
) -> Result<(), FormatError> {
    let mut buf = vec![0u8; usize::try_from(len.min(COPY_CHUNK)).unwrap_or(0)];
    let mut done = 0;
    while done < len {
        let n = usize::try_from((len - done).min(COPY_CHUNK)).unwrap_or(0);
        read_into(src, offset + done, &mut buf[..n])?;
        out.write_all(&buf[..n])?;
        done += n as u64;
    }
    Ok(())
}

/// Writes `len` zero bytes to `out`.
pub(crate) fn write_zeros(out: &mut (impl Write + ?Sized), len: u64) -> Result<(), FormatError> {
    std::io::copy(&mut std::io::repeat(0).take(len), out)?;
    Ok(())
}

/// How much [`copy_range`] reads at once.
pub(crate) const COPY_CHUNK: u64 = 4 << 20;

/// `n` rounded up to a multiple of `align`, a power of two.
pub(crate) fn align_up(n: u64, align: u64) -> u64 {
    n.next_multiple_of(align)
}

/// Length of the source in bytes, leaving the position at the start.
pub(crate) fn source_len(src: &mut (impl RomSource + ?Sized)) -> Result<u64, FormatError> {
    let len = src.seek(SeekFrom::End(0))?;
    src.seek(SeekFrom::Start(0))?;
    Ok(len)
}

/// ASCII text from a fixed-size header field, without trailing NULs and spaces.
pub fn ascii_field(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0 && b != b' ').map_or(0, |i| i + 1);
    bytes[..end]
        .iter()
        .map(|&b| if b.is_ascii() { char::from(b) } else { '?' })
        .collect()
}

/// The bytes `at..at + len` of `data`.
pub(crate) fn slice_at(data: &[u8], at: usize, len: usize) -> Result<&[u8], FormatError> {
    at.checked_add(len)
        .and_then(|end| data.get(at..end))
        .ok_or(FormatError::Truncated { offset: at as u64 })
}

/// The little-endian 16-bit value at `at`.
pub(crate) fn u16_at(data: &[u8], at: usize) -> Result<u16, FormatError> {
    let b = slice_at(data, at, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// The little-endian 32-bit value at `at`.
pub(crate) fn u32_at(data: &[u8], at: usize) -> Result<u32, FormatError> {
    let b = slice_at(data, at, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The little-endian 32-bit value at `at`, as an offset or size.
pub(crate) fn usize_at(data: &[u8], at: usize) -> Result<usize, FormatError> {
    u32_at(data, at).map(|v| v as usize)
}
