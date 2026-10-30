//! CurseForge 文件指纹：去掉空白字节（`\t`、`\n`、`\r`、空格）后的 MurmurHash2（32 位，种子 1）。
//!
//! 用于 `POST /fingerprints/432` 按文件内容反查所属项目与文件，相当于 Modrinth 的按 SHA1 查询。

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use tmcl_common::AppResult;

const M: u32 = 0x5bd1_e995;
const SEED: u32 = 1;

fn is_whitespace(b: u8) -> bool {
    matches!(b, 9 | 10 | 13 | 32)
}

/// 内存中数据的指纹。
pub fn of_bytes(data: &[u8]) -> u32 {
    let len = data.iter().filter(|&&b| !is_whitespace(b)).count();
    let mut hasher = Murmur2::new(len);
    for &b in data {
        if !is_whitespace(b) {
            hasher.push(b);
        }
    }
    hasher.finish()
}

/// 文件的指纹：读两遍，第一遍数出有效长度，第二遍计算，不把整个文件读进内存。
pub fn of_file(path: &Path) -> AppResult<u32> {
    let io = |e: std::io::Error| crate::install::io_error(path, &e);
    let mut reader = BufReader::new(File::open(path).map_err(io)?);
    let mut buf = [0u8; 64 * 1024];
    let mut len = 0usize;
    loop {
        let n = reader.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        len += buf[..n].iter().filter(|&&b| !is_whitespace(b)).count();
    }
    reader.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut hasher = Murmur2::new(len);
    loop {
        let n = reader.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if !is_whitespace(b) {
                hasher.push(b);
            }
        }
    }
    Ok(hasher.finish())
}

/// 逐字节输入的 MurmurHash2；长度需预先知道（参与初始值）。
struct Murmur2 {
    h: u32,
    block: [u8; 4],
    filled: usize,
}

impl Murmur2 {
    fn new(len: usize) -> Self {
        // CurseForge 的实现把长度截成 32 位。
        Self {
            h: SEED ^ (len as u32),
            block: [0; 4],
            filled: 0,
        }
    }

    fn push(&mut self, b: u8) {
        self.block[self.filled] = b;
        self.filled += 1;
        if self.filled == 4 {
            let mut k = u32::from_le_bytes(self.block);
            k = k.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            self.h = self.h.wrapping_mul(M) ^ k;
            self.filled = 0;
        }
    }

    fn finish(mut self) -> u32 {
        let tail = &self.block[..self.filled];
        if !tail.is_empty() {
            if tail.len() >= 3 {
                self.h ^= u32::from(tail[2]) << 16;
            }
            if tail.len() >= 2 {
                self.h ^= u32::from(tail[1]) << 8;
            }
            self.h ^= u32::from(tail[0]);
            self.h = self.h.wrapping_mul(M);
        }
        let mut h = self.h;
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 标准 MurmurHash2（不去空白）的参考实现，用于对照逐字节版本。
    fn reference(data: &[u8], seed: u32) -> u32 {
        let mut h = seed ^ data.len() as u32;
        let (chunks, r) = data.as_chunks::<4>();
        for c in chunks {
            let mut k = u32::from_le_bytes(*c);
            k = k.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            h = h.wrapping_mul(M) ^ k;
        }
        if r.len() == 3 {
            h ^= u32::from(r[2]) << 16;
        }
        if r.len() >= 2 {
            h ^= u32::from(r[1]) << 8;
        }
        if !r.is_empty() {
            h ^= u32::from(r[0]);
            h = h.wrapping_mul(M);
        }
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^ (h >> 15)
    }

    #[test]
    fn matches_reference_for_all_tail_lengths() {
        for len in 0..12 {
            let data: Vec<u8> = (0..len).map(|i| b'a' + i).collect();
            assert_eq!(of_bytes(&data), reference(&data, SEED), "len {len}");
        }
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(of_bytes(b"a b\tc\r\nd"), of_bytes(b"abcd"));
        assert_ne!(of_bytes(b"abcd"), of_bytes(b"abce"));
    }

    #[test]
    fn file_and_bytes_agree() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 256) as u8).collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.jar");
        std::fs::write(&path, &data).unwrap();
        assert_eq!(of_file(&path).unwrap(), of_bytes(&data));
    }
}
