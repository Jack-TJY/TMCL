//! 按键值在两种格式间的转换。
//!
//! - 1.13 及以后（[`KeyFormat::Named`]）：`key.keyboard.w`、`key.mouse.left` 形式的键名。
//! - 1.12.2 及以前（[`KeyFormat::Lwjgl2`]）：LWJGL2 数字键码，`0` 为未绑定，
//!   鼠标按钮为 `-100 + 按钮序号`（左键 `-100`、右键 `-99`、中键 `-98`）。
//!
//! 键盘的对应关系来自 `data/keymap-lwjgl2-glfw.json`。按键值后可能带 Forge 的修饰键后缀
//! （如 `17:CONTROL`），转换时原样保留。无法转换的值（模组自定义输入、扫描码等）原样保留。

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

/// 按键值的存储格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    /// 1.13 及以后的键名。
    Named,
    /// 1.12.2 及以前的 LWJGL2 数字键码。
    Lwjgl2,
}

/// 鼠标键名前缀。
const MOUSE_PREFIX: &str = "key.mouse.";
/// LWJGL2 中鼠标按钮键码的起点。
const MOUSE_BASE: i32 = -100;
/// 支持换算的鼠标按钮数。
const MOUSE_BUTTONS: i32 = 16;

#[derive(Deserialize)]
struct TableFile {
    keyboard: HashMap<String, i32>,
}

/// 键盘键名与键码的双向表。
struct KeyTable {
    by_name: HashMap<String, i32>,
    by_code: HashMap<i32, String>,
}

static TABLE: LazyLock<KeyTable> = LazyLock::new(|| {
    // 数据随源码编译进来，由单元测试保证可以解析。
    let file: TableFile = serde_json::from_str(include_str!("../data/keymap-lwjgl2-glfw.json"))
        .unwrap_or_else(|_| TableFile {
            keyboard: HashMap::new(),
        });
    let by_code = file
        .keyboard
        .iter()
        .map(|(name, code)| (*code, name.clone()))
        .collect();
    KeyTable {
        by_name: file.keyboard,
        by_code,
    }
});

/// 键名对应的 LWJGL2 键码（含鼠标键）；不认识的键名返回 `None`。
pub fn code_for_name(name: &str) -> Option<i32> {
    if let Some(button) = name.strip_prefix(MOUSE_PREFIX) {
        let index = match button {
            "left" => 0,
            "right" => 1,
            "middle" => 2,
            n => {
                n.parse::<i32>()
                    .ok()
                    .filter(|n| (4..=MOUSE_BUTTONS).contains(n))?
                    - 1
            }
        };
        return Some(MOUSE_BASE + index);
    }
    TABLE.by_name.get(name).copied()
}

/// LWJGL2 键码对应的键名（含鼠标键）；表中没有的键码返回 `None`。
pub fn name_for_code(code: i32) -> Option<String> {
    if (MOUSE_BASE..MOUSE_BASE + MOUSE_BUTTONS).contains(&code) {
        return Some(match code - MOUSE_BASE {
            0 => format!("{MOUSE_PREFIX}left"),
            1 => format!("{MOUSE_PREFIX}right"),
            2 => format!("{MOUSE_PREFIX}middle"),
            i => format!("{MOUSE_PREFIX}{}", i + 1),
        });
    }
    TABLE.by_code.get(&code).cloned()
}

/// 把一个按键绑定的值转换为 `to` 格式；已是目标格式或无法转换时原样返回。
pub fn convert_value(value: &str, to: KeyFormat) -> String {
    let (key, suffix) = match value.split_once(':') {
        Some((key, rest)) => (key, Some(rest)),
        None => (value, None),
    };
    let converted = match to {
        KeyFormat::Lwjgl2 => code_for_name(key).map(|code| code.to_string()),
        KeyFormat::Named => key.parse::<i32>().ok().and_then(name_for_code),
    };
    match (converted, suffix) {
        (Some(key), Some(suffix)) => format!("{key}:{suffix}"),
        (Some(key), None) => key,
        (None, _) => value.to_owned(),
    }
}

/// 从按键值本身判断格式：数字为 LWJGL2，`key.` 开头为键名，其他无法判断。
pub fn detect_value_format(value: &str) -> Option<KeyFormat> {
    let key = value.split_once(':').map_or(value, |(key, _)| key);
    if key.parse::<i32>().is_ok() {
        Some(KeyFormat::Lwjgl2)
    } else if key.starts_with("key.") {
        Some(KeyFormat::Named)
    } else {
        None
    }
}

/// 游戏版本号是否不早于给定版本：`release` 为 `1.x` 中的 `x`，`snapshot` 为对应的
/// 快照周 `(年, 周)`。远古版本（`a1.`、`b1.`、`rd-` 等）为 `false`，无法识别时为 `None`。
pub fn version_at_least(version: &str, release: u32, snapshot: (u32, u32)) -> Option<bool> {
    let v = version.trim();
    let ancient = ["a", "b", "c", "rd-", "inf-"].iter().any(|p| {
        v.strip_prefix(p)
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    });
    if ancient {
        return Some(false);
    }
    // 快照：`23w31a`
    let bytes = v.as_bytes();
    if v.is_ascii() && bytes.len() >= 6 && bytes[2] == b'w' {
        let year = v[..2].parse::<u32>().ok()?;
        let week = v[3..5].parse::<u32>().ok()?;
        return Some((year, week) >= snapshot);
    }
    // 正式版与预发布：`1.20.1`、`1.13-pre1`；`1.` 之后的年份制版本号一律视为更新。
    let mut parts = v.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse::<u32>().ok()?;
    if major != 1 {
        return Some(major > 1);
    }
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some(minor >= release)
}

/// 游戏版本使用的按键格式（1.13 / 17w43a 起为键名）。
pub fn format_for_version(version: &str) -> Option<KeyFormat> {
    version_at_least(version, 13, (17, 43)).map(|named| {
        if named {
            KeyFormat::Named
        } else {
            KeyFormat::Lwjgl2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_entry_converts_both_ways() {
        let table = &*TABLE;
        assert!(table.by_name.len() > 100, "映射表未能加载");
        assert_eq!(table.by_name.len(), table.by_code.len(), "键码重复");
        for (name, code) in &table.by_name {
            assert_eq!(code_for_name(name), Some(*code), "{name}");
            assert_eq!(
                name_for_code(*code).as_deref(),
                Some(name.as_str()),
                "{code}"
            );
            assert_eq!(convert_value(name, KeyFormat::Lwjgl2), code.to_string());
            assert_eq!(convert_value(&code.to_string(), KeyFormat::Named), *name);
        }
    }

    #[test]
    fn table_covers_all_standard_keys() {
        let mut names: Vec<String> = ('a'..='z')
            .chain('0'..='9')
            .map(|c| format!("key.keyboard.{c}"))
            .collect();
        names.extend((1..=19).map(|n| format!("key.keyboard.f{n}")));
        names.extend((0..=9).map(|n| format!("key.keyboard.keypad.{n}")));
        for rest in [
            "unknown",
            "keypad.add",
            "keypad.decimal",
            "keypad.divide",
            "keypad.enter",
            "keypad.equal",
            "keypad.multiply",
            "keypad.subtract",
            "up",
            "down",
            "left",
            "right",
            "apostrophe",
            "backslash",
            "comma",
            "equal",
            "grave.accent",
            "left.bracket",
            "minus",
            "period",
            "right.bracket",
            "semicolon",
            "slash",
            "space",
            "tab",
            "enter",
            "escape",
            "backspace",
            "delete",
            "insert",
            "home",
            "end",
            "page.up",
            "page.down",
            "caps.lock",
            "num.lock",
            "scroll.lock",
            "print.screen",
            "pause",
            "menu",
            "left.shift",
            "right.shift",
            "left.control",
            "right.control",
            "left.alt",
            "right.alt",
            "left.win",
            "right.win",
        ] {
            names.push(format!("key.keyboard.{rest}"));
        }
        for name in names {
            assert!(code_for_name(&name).is_some(), "缺少 {name}");
        }
        assert_eq!(code_for_name("key.keyboard.unknown"), Some(0));
        assert_eq!(code_for_name("key.keyboard.w"), Some(17));
    }

    #[test]
    fn mouse_buttons_convert_separately() {
        let cases = [
            ("key.mouse.left", -100),
            ("key.mouse.right", -99),
            ("key.mouse.middle", -98),
            ("key.mouse.4", -97),
            ("key.mouse.16", -85),
        ];
        for (name, code) in cases {
            assert_eq!(code_for_name(name), Some(code));
            assert_eq!(name_for_code(code).as_deref(), Some(name));
        }
        assert_eq!(code_for_name("key.mouse.17"), None);
        assert_eq!(code_for_name("key.mouse.3"), None);
        assert_eq!(name_for_code(-84), None);
    }

    #[test]
    fn unknown_values_and_modifiers() {
        assert_eq!(
            convert_value("17:CONTROL", KeyFormat::Named),
            "key.keyboard.w:CONTROL"
        );
        assert_eq!(
            convert_value("key.keyboard.w:SHIFT", KeyFormat::Lwjgl2),
            "17:SHIFT"
        );
        assert_eq!(
            convert_value("scancode.300", KeyFormat::Lwjgl2),
            "scancode.300"
        );
        assert_eq!(convert_value("9999", KeyFormat::Named), "9999");
        assert_eq!(
            convert_value("key.keyboard.w", KeyFormat::Named),
            "key.keyboard.w"
        );
        assert_eq!(detect_value_format("-100"), Some(KeyFormat::Lwjgl2));
        assert_eq!(
            detect_value_format("key.mouse.left"),
            Some(KeyFormat::Named)
        );
        assert_eq!(detect_value_format("x"), None);
    }

    #[test]
    fn version_formats() {
        use KeyFormat::*;
        for (v, f) in [
            ("1.20.1", Some(Named)),
            ("1.13", Some(Named)),
            ("1.13-pre1", Some(Named)),
            ("26.1", Some(Named)),
            ("17w43a", Some(Named)),
            ("23w31a", Some(Named)),
            ("1.12.2", Some(Lwjgl2)),
            ("1.7.10", Some(Lwjgl2)),
            ("17w31a", Some(Lwjgl2)),
            ("b1.7.3", Some(Lwjgl2)),
            ("rd-132211", Some(Lwjgl2)),
            ("custom", None),
        ] {
            assert_eq!(format_for_version(v), f, "{v}");
        }
    }
}
