//! Binary property lists that hold one string.
//!
//! Finder stores a folder comment as a `bplist00` string in the extended
//! attribute `com.apple.metadata:kMDItemFinderComment`. Spotlight indexes that
//! attribute. The encoder writes the same object layout Finder writes, and the
//! decoder reads it back, including comments Finder itself saved.

const HEADER: &[u8] = b"bplist00";
const TRAILER_LEN: usize = 32;
/// Spotlight keeps a finder comment only while the attribute stays inline.
/// Stay under the inline ceiling (about 3804 bytes) with room for the plist.
pub const MAX_PLIST_BYTES: usize = 3700;

#[derive(Debug, PartialEq, Eq)]
pub struct PlistError(pub String);

pub fn encode_string(text: &str) -> Result<Vec<u8>, PlistError> {
    let ascii = text.is_ascii();
    let count = if ascii {
        text.len()
    } else {
        text.encode_utf16().count()
    };
    let mut object = Vec::new();
    let marker = if ascii { 0x50 } else { 0x60 };
    if count < 15 {
        object.push(marker | count as u8);
    } else {
        object.push(marker | 0x0f);
        write_length(&mut object, count)?;
    }
    if ascii {
        object.extend_from_slice(text.as_bytes());
    } else {
        for unit in text.encode_utf16() {
            object.extend_from_slice(&unit.to_be_bytes());
        }
    }

    // The single object always starts at byte 8, so the offset table is one byte.
    let table_offset = 8 + object.len();
    let mut out = Vec::with_capacity(table_offset + 1 + TRAILER_LEN);
    out.extend_from_slice(HEADER);
    out.extend_from_slice(&object);
    out.push(8);

    let mut trailer = [0u8; TRAILER_LEN];
    trailer[6] = 1;
    trailer[7] = 1;
    trailer[15] = 1;
    trailer[24..32].copy_from_slice(&(table_offset as u64).to_be_bytes());
    out.extend_from_slice(&trailer);

    if out.len() > MAX_PLIST_BYTES {
        return Err(PlistError(format!(
            "That note is too long for Spotlight to keep on a folder ({} bytes of metadata, limit {}).",
            out.len(),
            MAX_PLIST_BYTES
        )));
    }
    Ok(out)
}

pub fn decode_string(bytes: &[u8]) -> Result<String, PlistError> {
    if bytes.len() < HEADER.len() + TRAILER_LEN || &bytes[..HEADER.len()] != HEADER {
        return Err(PlistError(
            "The note on this folder is not a Finder comment.".into(),
        ));
    }
    let trailer = &bytes[bytes.len() - TRAILER_LEN..];
    let offset_size = trailer[6] as usize;
    let object_count = read_be(&trailer[8..16])?;
    let top = read_be(&trailer[16..24])?;
    let table_offset = read_be(&trailer[24..32])? as usize;
    if offset_size == 0 || offset_size > 8 || top >= object_count {
        return Err(PlistError(
            "The note on this folder is not a Finder comment.".into(),
        ));
    }
    let entry = table_offset
        .checked_add(top as usize * offset_size)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    let object_offset = read_sized(bytes, entry, offset_size)? as usize;
    read_string(bytes, object_offset)
}

fn write_length(out: &mut Vec<u8>, count: usize) -> Result<(), PlistError> {
    if count <= u8::MAX as usize {
        out.push(0x10);
        out.push(count as u8);
        return Ok(());
    }
    if count <= u16::MAX as usize {
        out.push(0x11);
        out.extend_from_slice(&(count as u16).to_be_bytes());
        return Ok(());
    }
    if count <= u32::MAX as usize {
        out.push(0x12);
        out.extend_from_slice(&(count as u32).to_be_bytes());
        return Ok(());
    }
    Err(PlistError(
        "That note is too long for Spotlight to keep on a folder.".into(),
    ))
}

fn read_string(bytes: &[u8], start: usize) -> Result<String, PlistError> {
    let marker = *bytes
        .get(start)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    let kind = marker >> 4;
    if kind != 0x5 && kind != 0x6 {
        return Err(PlistError(
            "The note on this folder is not a Finder comment.".into(),
        ));
    }
    let (count, mut cursor) = read_count(bytes, start, marker & 0x0f)?;
    if kind == 0x5 {
        let end = cursor
            .checked_add(count)
            .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
        let slice = bytes
            .get(cursor..end)
            .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
        return String::from_utf8(slice.to_vec())
            .map_err(|_| PlistError("The note on this folder is not valid text.".into()));
    }
    let byte_len = count
        .checked_mul(2)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    let end = cursor
        .checked_add(byte_len)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    if bytes.get(cursor..end).is_none() {
        return Err(PlistError("The note on this folder is unreadable.".into()));
    }
    let mut units = Vec::with_capacity(count);
    while cursor < end {
        let pair = bytes
            .get(cursor..cursor + 2)
            .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
        units.push(u16::from_be_bytes([pair[0], pair[1]]));
        cursor += 2;
    }
    String::from_utf16(&units)
        .map_err(|_| PlistError("The note on this folder is not valid text.".into()))
}

fn read_count(bytes: &[u8], start: usize, nibble: u8) -> Result<(usize, usize), PlistError> {
    if nibble != 0x0f {
        return Ok((nibble as usize, start + 1));
    }
    let (value, next) = read_int(bytes, start + 1)?;
    Ok((value as usize, next))
}

fn read_int(bytes: &[u8], start: usize) -> Result<(u64, usize), PlistError> {
    let marker = *bytes
        .get(start)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    if marker >> 4 != 0x1 || (marker & 0x0f) > 3 {
        return Err(PlistError(
            "The note on this folder is not a Finder comment.".into(),
        ));
    }
    let size = 1usize << (marker & 0x0f);
    let value = read_sized(bytes, start + 1, size)?;
    Ok((value, start + 1 + size))
}

fn read_sized(bytes: &[u8], start: usize, size: usize) -> Result<u64, PlistError> {
    let end = start
        .checked_add(size)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    let slice = bytes
        .get(start..end)
        .ok_or_else(|| PlistError("The note on this folder is unreadable.".into()))?;
    read_be(slice)
}

fn read_be(bytes: &[u8]) -> Result<u64, PlistError> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err(PlistError("The note on this folder is unreadable.".into()));
    }
    let mut value = 0u64;
    for byte in bytes {
        value = (value << 8) | u64::from(*byte);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn decodes_a_finder_comment_captured_from_this_mac() {
        // `osascript` set this Finder comment, then `xattr -px` printed these bytes.
        let hex = "62706c69737430306f1015005000720061006800610020201400200022005100330022000a006c0069006e0065002000740077006f080000000000000101000000000000000100000000000000000000000000000035";
        let bytes = decode_hex(hex);
        let text = decode_string(&bytes).unwrap();
        assert_eq!(text, "Praha — \"Q3\"\nline two");
    }

    #[test]
    fn roundtrips_through_the_system_plist_parser() {
        let samples = [
            "",
            "hi",
            "fourteen chars",
            "fifteen charss!",
            "a]256",
            "Praha — \"Q3\"\nline two",
            "emoji \u{1F44B} and ascii",
        ];
        for sample in samples {
            let sample = if sample == "a]256" {
                "a".repeat(256)
            } else {
                sample.to_string()
            };
            let encoded = encode_string(&sample).unwrap();
            assert_eq!(decode_string(&encoded).unwrap(), sample);
            assert_eq!(python_loads(&encoded), sample);
            assert_eq!(decode_string(&python_dumps(&sample)).unwrap(), sample);
        }
    }

    #[test]
    fn rejects_a_truncated_plist() {
        let err = decode_string(b"bplist00").unwrap_err();
        assert!(err.0.contains("not a Finder comment"));
    }

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }

    /// The system plist parser is Python's `plistlib`. Windows runners ship
    /// `python`, not `python3`.
    fn python() -> Command {
        for name in ["python3", "python"] {
            let found = Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success());
            if found {
                return Command::new(name);
            }
        }
        panic!("these tests need python3 (or python) on PATH for plistlib");
    }

    fn python_loads(bytes: &[u8]) -> String {
        let mut child = python()
            .arg("-c")
            .arg("import plistlib,sys; sys.stdout.write(plistlib.loads(sys.stdin.buffer.read()))")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn python_dumps(text: &str) -> Vec<u8> {
        let mut child = python()
            .arg("-c")
            .arg(
                "import io,plistlib,sys; buf=io.BytesIO(); plistlib.dump(sys.stdin.read(), buf, fmt=plistlib.FMT_BINARY); sys.stdout.buffer.write(buf.getvalue())",
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}
