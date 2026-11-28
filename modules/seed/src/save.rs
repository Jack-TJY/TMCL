//! Read the world seed from a saved Java Edition `level.dat`.
//!
//! 1.12.2 stores the seed in `Data.RandomSeed`; 1.16+ moved it to
//! `Data.WorldGenSettings.seed`. Only the tags needed for the seed, level name
//! and version are interpreted; everything else is skipped.
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use tmcl_common::{AppError, AppResult, ErrorCode};

const MAX_LEVEL_DAT: u64 = 64 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveInfo {
    pub name: String,
    pub seed: i64,
    pub version: Option<String>,
}

#[derive(Default)]
struct Extracted {
    seed: Option<i64>,
    name: Option<String>,
    version: Option<String>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> AppResult<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(corrupt)?;
        if end > self.bytes.len() {
            return Err(corrupt());
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
    fn u8(&mut self) -> AppResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> AppResult<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> AppResult<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i64(&mut self) -> AppResult<i64> {
        let b = self.take(8)?;
        Ok(i64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn string(&mut self) -> AppResult<String> {
        let len = usize::from(self.u16()?);
        let b = self.take(len)?;
        String::from_utf8(b.to_vec()).map_err(|_| corrupt())
    }
}

fn corrupt() -> AppError {
    AppError::new(ErrorCode::SeedInvalidInput).with_param("field", "save")
}

fn capture(extracted: &mut Extracted, path: &[String], value: &Value) {
    let joined = path.join(".");
    match (joined.as_str(), value) {
        ("Data.RandomSeed", Value::Long(v)) => {
            extracted.seed.get_or_insert(*v);
        }
        ("Data.WorldGenSettings.seed", Value::Long(v)) => {
            extracted.seed.get_or_insert(*v);
        }
        ("Data.LevelName", Value::String(s)) => {
            extracted.name.get_or_insert_with(|| s.clone());
        }
        ("Data.Version.Name", Value::String(s)) => {
            extracted.version.get_or_insert_with(|| s.clone());
        }
        _ => {}
    }
}

enum Value {
    Long(i64),
    String(String),
}

fn parse_payload(
    tag: u8,
    reader: &mut Reader<'_>,
    path: &mut Vec<String>,
    extracted: &mut Extracted,
) -> AppResult<()> {
    match tag {
        1 => {
            reader.take(1)?;
        }
        2 => {
            reader.take(2)?;
        }
        3 => {
            reader.take(4)?;
        }
        4 => {
            let value = reader.i64()?;
            capture(extracted, path, &Value::Long(value));
        }
        5 => {
            reader.take(4)?;
        }
        6 => {
            reader.take(8)?;
        }
        7 => {
            let len = reader.i32()?;
            if len < 0 {
                return Err(corrupt());
            }
            reader.take(len as usize)?;
        }
        8 => {
            let value = reader.string()?;
            capture(extracted, path, &Value::String(value));
        }
        9 => {
            let inner = reader.u8()?;
            let len = reader.i32()?;
            if len < 0 {
                return Err(corrupt());
            }
            for _ in 0..len {
                if inner == 0 {
                    return Err(corrupt());
                }
                parse_payload(inner, reader, path, extracted)?;
            }
        }
        10 => loop {
            let inner = reader.u8()?;
            if inner == 0 {
                break;
            }
            let name = reader.string()?;
            path.push(name);
            parse_payload(inner, reader, path, extracted)?;
            path.pop();
        },
        11 => {
            let len = reader.i32()?;
            if len < 0 {
                return Err(corrupt());
            }
            reader.take(len as usize * 4)?;
        }
        12 => {
            let len = reader.i32()?;
            if len < 0 {
                return Err(corrupt());
            }
            reader.take(len as usize * 8)?;
        }
        _ => return Err(corrupt()),
    }
    Ok(())
}

/// Parse one `level.dat`; the caller resolves the instance-relative path.
pub fn read_level_dat(path: &Path) -> AppResult<SaveInfo> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::from(e).with_param("path", path.display()))?;
    let mut raw = Vec::new();
    GzDecoder::new(file)
        .take(MAX_LEVEL_DAT)
        .read_to_end(&mut raw)
        .map_err(|e| AppError::from(e).with_param("path", path.display()))?;
    let mut reader = Reader {
        bytes: &raw,
        pos: 0,
    };
    if reader.u8()? != 10 {
        return Err(corrupt());
    }
    let _root_name = reader.string()?;
    let mut extracted = Extracted::default();
    parse_payload(10, &mut reader, &mut Vec::new(), &mut extracted)?;
    Ok(SaveInfo {
        name: extracted.name.unwrap_or_default(),
        seed: extracted.seed.ok_or_else(corrupt)?,
        version: extracted.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tag(out: &mut Vec<u8>, kind: u8, name: &str) {
        out.push(kind);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    fn long(out: &mut Vec<u8>, name: &str, value: i64) {
        tag(out, 4, name);
        out.extend_from_slice(&value.to_be_bytes());
    }
    fn string(out: &mut Vec<u8>, name: &str, value: &str) {
        tag(out, 8, name);
        out.extend_from_slice(&(value.len() as u16).to_be_bytes());
        out.extend_from_slice(value.as_bytes());
    }
    fn compound(out: &mut Vec<u8>, name: &str, body: impl FnOnce(&mut Vec<u8>)) {
        tag(out, 10, name);
        body(out);
        out.push(0);
    }
    fn write_save(dir: &Path, root: Vec<u8>) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let file = dir.join("level.dat");
        let mut encoder = flate2::write::GzEncoder::new(
            std::fs::File::create(&file).unwrap(),
            flate2::Compression::fast(),
        );
        encoder.write_all(&root).unwrap();
        encoder.finish().unwrap();
        file
    }
    fn root(body: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut out = Vec::new();
        tag(&mut out, 10, "");
        body(&mut out);
        out.push(0);
        out
    }

    #[test]
    fn reads_112_seed_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_save(
            dir.path(),
            root(|out| {
                compound(out, "Data", |out| {
                    long(out, "RandomSeed", -1);
                    string(out, "LevelName", "Old World");
                    compound(out, "Version", |out| string(out, "Name", "1.12.2"));
                });
            }),
        );
        assert_eq!(
            read_level_dat(&path).unwrap(),
            SaveInfo {
                name: "Old World".into(),
                seed: -1,
                version: Some("1.12.2".into())
            }
        );
    }

    #[test]
    fn reads_modern_seed_from_worldgen_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_save(
            dir.path(),
            root(|out| {
                compound(out, "Data", |out| {
                    string(out, "LevelName", "New World");
                    compound(out, "WorldGenSettings", |out| long(out, "seed", 42));
                    compound(out, "Version", |out| string(out, "Name", "26.3"));
                });
            }),
        );
        assert_eq!(
            read_level_dat(&path).unwrap(),
            SaveInfo {
                name: "New World".into(),
                seed: 42,
                version: Some("26.3".into())
            }
        );
    }

    #[test]
    fn rejects_missing_seed_and_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let no_seed = write_save(
            &dir.path().join("a"),
            root(|out| compound(out, "Data", |out| string(out, "LevelName", "x"))),
        );
        assert!(read_level_dat(&no_seed).is_err());
        let file = dir.path().join("bad.dat");
        std::fs::write(&file, b"not gzip").unwrap();
        assert!(read_level_dat(&file).is_err());
    }
}
