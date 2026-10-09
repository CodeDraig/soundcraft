//! Audio container identification by magic and extension.

use super::{FileFormat, aiff, wav};

/// Size of an ID3v2 tag at the start of `b`, if there is one.
fn id3v2_len(b: &[u8]) -> Option<usize> {
    if b.get(..3)? != b"ID3" {
        return None;
    }
    let flags = *b.get(5)?;
    let size = b.get(6..10)?.iter().fold(0usize, |acc, &x| (acc << 7) | usize::from(x & 0x7F));
    Some(10 + size + if flags & 0x10 != 0 { 10 } else { 0 })
}

fn detect_by_magic(b: &[u8], depth: u32) -> Option<FileFormat> {
    let head = |n: usize| b.get(..n);
    if wav::is_wav(b) {
        return Some(FileFormat::Wav);
    }
    if aiff::is_aiff(b) {
        return Some(FileFormat::Aiff);
    }
    match head(4)? {
        b"fLaC" => return Some(FileFormat::Flac),
        b"OggS" => return Some(FileFormat::Ogg),
        b"caff" => return Some(FileFormat::Caf),
        [0x1A, 0x45, 0xDF, 0xA3] => return Some(FileFormat::Other),
        _ => {}
    }
    if b.get(4..8) == Some(b"ftyp") {
        // MP4 / M4A: the sample entry inside `stsd` names the codec.
        let found = b.windows(4).position(|w| w == b"stsd");
        let codec = found.and_then(|p| b.get(p.checked_add(16)?..p.checked_add(20)?));
        return Some(if codec == Some(b"alac") { FileFormat::Alac } else { FileFormat::Aac });
    }
    if let Some(len) = id3v2_len(b) {
        if depth == 0
            && let Some(f) = b.get(len..).and_then(|rest| detect_by_magic(rest, 1))
        {
            return Some(f);
        }
        return Some(FileFormat::Mp3);
    }
    let (b0, b1) = (*b.first()?, *b.get(1)?);
    if b0 == 0xFF && b1 & 0xE0 == 0xE0 {
        let layer = (b1 >> 1) & 0x3;
        if b1 & 0xF0 == 0xF0 && layer == 0 {
            return Some(FileFormat::Aac); // ADTS
        }
        if layer != 0 {
            return Some(FileFormat::Mp3);
        }
    }
    None
}

fn detect_by_ext(ext: &str) -> FileFormat {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "wav" | "wave" | "bwf" | "rf64" => FileFormat::Wav,
        "aif" | "aiff" | "aifc" => FileFormat::Aiff,
        "flac" => FileFormat::Flac,
        "mp3" | "mp2" | "mp1" => FileFormat::Mp3,
        "ogg" | "oga" => FileFormat::Ogg,
        "aac" | "m4a" | "mp4" | "adts" => FileFormat::Aac,
        "caf" => FileFormat::Caf,
        _ => FileFormat::Other,
    }
}

/// Identify a file by its magic bytes, falling back to the extension hint (`"wav"`, `".flac"`).
pub fn detect_format(bytes: &[u8], ext_hint: Option<&str>) -> FileFormat {
    detect_by_magic(bytes, 0).unwrap_or_else(|| ext_hint.map(detect_by_ext).unwrap_or(FileFormat::Other))
}
