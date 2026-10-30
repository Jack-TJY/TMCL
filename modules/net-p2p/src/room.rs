//! Scaffolding 房间码（与 HMCL、PCL-CE、陶瓦联机互通）。
//!
//! 规范见 <https://github.com/Scaffolding-MC/Scaffolding-MC>（MIT）：
//!
//! - 房间码形如 `U/NNNN-NNNN-SSSS-SSSS`，字符取自 `0-9`、`A-H`、`J-N`、`P-Z`
//!   （去掉 I 和 O），依次映射为 0–33；
//! - 16 个字符按“小端序”读成 34 进制整数（第一个字符是最低位），该整数必须能被 7 整除；
//! - EasyTier 网络名为 `scaffolding-mc-NNNN-NNNN`，网络密钥为 `SSSS-SSSS`；
//! - 联机中心（房主）的 EasyTier 主机名为 `scaffolding-mc-server-<端口>`，端口须在 1025–65535。

use std::fmt;

use tmcl_common::{AppError, AppResult, ErrorCode};

/// 房间码字符表，下标即字符的值。
const ALPHABET: &[u8; 34] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
/// 房间码有效字符数（不含 `U/` 与分隔符）。
const DIGITS: usize = 16;
/// 房间码前缀。
const PREFIX: &str = "U/";
/// 联机中心主机名前缀。
const CENTER_PREFIX: &str = "scaffolding-mc-server-";

/// 一个 Scaffolding 房间：房间码及由它导出的 EasyTier 网络名和密钥。
///
/// 房间码本身就是网络密钥，[`fmt::Debug`] 只输出网络名，避免写进日志。
#[derive(Clone, PartialEq, Eq)]
pub struct RoomCode {
    code: String,
    network_name: String,
    network_secret: String,
}

impl RoomCode {
    /// 用操作系统的密码学安全随机数生成一个新房间码。
    pub fn generate() -> AppResult<Self> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|e| {
            AppError::new(ErrorCode::CommonInternal).with_detail(format!("getrandom: {e}"))
        })?;
        let value = u128::from_le_bytes(bytes) % 34u128.pow(DIGITS as u32);
        Ok(Self::from_value(value - value % 7))
    }

    /// 解析用户输入的房间码。忽略首尾空白和大小写，`U/` 前缀可省略。
    pub fn parse(input: &str) -> AppResult<Self> {
        let invalid = || AppError::new(ErrorCode::P2pRoomCodeInvalid);
        let upper = input.trim().to_ascii_uppercase();
        let body = upper.strip_prefix(PREFIX).unwrap_or(&upper);
        let groups: Vec<&str> = body.split('-').collect();
        if groups.len() != 4 || groups.iter().any(|g| g.len() != 4) {
            return Err(invalid());
        }
        let mut value: u128 = 0;
        for (i, ch) in groups.concat().bytes().enumerate() {
            let digit = ALPHABET.iter().position(|&c| c == ch).ok_or_else(invalid)?;
            value += digit as u128 * 34u128.pow(i as u32);
        }
        if !value.is_multiple_of(7) {
            return Err(invalid());
        }
        Ok(Self::from_value(value))
    }

    fn from_value(mut value: u128) -> Self {
        let mut chars = String::with_capacity(DIGITS);
        for _ in 0..DIGITS {
            chars.push(ALPHABET[(value % 34) as usize] as char);
            value /= 34;
        }
        let (name, secret) = chars.split_at(DIGITS / 2);
        let name = format!("{}-{}", &name[..4], &name[4..]);
        let secret = format!("{}-{}", &secret[..4], &secret[4..]);
        Self {
            code: format!("{PREFIX}{name}-{secret}"),
            network_name: format!("scaffolding-mc-{name}"),
            network_secret: secret,
        }
    }

    /// 完整房间码 `U/NNNN-NNNN-SSSS-SSSS`。
    pub fn code(&self) -> &str {
        &self.code
    }

    /// EasyTier 网络名 `scaffolding-mc-NNNN-NNNN`。
    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    /// EasyTier 网络密钥 `SSSS-SSSS`。
    pub fn network_secret(&self) -> &str {
        &self.network_secret
    }
}

impl fmt::Debug for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoomCode")
            .field("network_name", &self.network_name)
            .finish_non_exhaustive()
    }
}

/// 联机中心的 EasyTier 主机名，`port` 是房主上 Scaffolding 信令服务的端口。
pub fn center_hostname(port: u16) -> String {
    format!("{CENTER_PREFIX}{port}")
}

/// 从 EasyTier 主机名中读出联机中心的信令端口；不是合法的联机中心时返回 `None`。
pub fn parse_center_hostname(hostname: &str) -> Option<u16> {
    let digits = hostname.strip_prefix(CENTER_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u16>().ok().filter(|&port| port > 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spec_example() {
        let room = RoomCode::parse("U/YNZE-U61D-2206-HXRG").unwrap();
        assert_eq!(room.code(), "U/YNZE-U61D-2206-HXRG");
        assert_eq!(room.network_name(), "scaffolding-mc-YNZE-U61D");
        assert_eq!(room.network_secret(), "2206-HXRG");
    }

    #[test]
    fn parse_is_lenient_about_case_prefix_and_whitespace() {
        let room = RoomCode::parse("  ynze-u61d-2206-hxrg\n").unwrap();
        assert_eq!(room.code(), "U/YNZE-U61D-2206-HXRG");
    }

    #[test]
    fn rejects_bad_checksum_and_format() {
        for input in [
            "U/YNZE-U61D-2206-HXRH",
            "U/YNZE-U61D-2206",
            "U/YNZE-U61D-2206-HXRGG",
            "U/YNZEU61D2206HXRG",
            "U/YNZE-U61D-22O6-HXRG",
            "U/YNZE-U61D-2206-HXR!",
            "",
        ] {
            let err = RoomCode::parse(input).unwrap_err();
            assert_eq!(err.code, ErrorCode::P2pRoomCodeInvalid, "{input}");
        }
    }

    #[test]
    fn generated_codes_round_trip() {
        for _ in 0..200 {
            let room = RoomCode::generate().unwrap();
            assert_eq!(room.code().len(), "U/XXXX-XXXX-XXXX-XXXX".len());
            assert!(!room.code().contains(['I', 'O']));
            assert_eq!(RoomCode::parse(room.code()).unwrap(), room);
        }
    }

    #[test]
    fn debug_hides_secret() {
        let room = RoomCode::parse("U/YNZE-U61D-2206-HXRG").unwrap();
        let debug = format!("{room:?}");
        assert!(debug.contains("scaffolding-mc-YNZE-U61D"));
        assert!(!debug.contains("2206"));
    }

    #[test]
    fn center_hostname_round_trip() {
        assert_eq!(center_hostname(33768), "scaffolding-mc-server-33768");
        assert_eq!(
            parse_center_hostname("scaffolding-mc-server-33768"),
            Some(33768)
        );
        for bad in [
            "scaffolding-mc-server-77844",
            "scaffolding-mc-server-n8",
            "scaffolding-mc-server-1024",
            "scaffolding-mc-server-",
            "scaffolding-mc-server-+2000",
            "other-33768",
        ] {
            assert_eq!(parse_center_hostname(bad), None, "{bad}");
        }
    }
}
