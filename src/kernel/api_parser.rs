//! .api 契约解析器（D6）
//!
//! 实际契约文件为 JSON（`api_definitions/*.api`），与任务书描述的 key=value
//! 行级语法不一致，以真实文件格式为准（已记入 FIX_REPORT 文档疑点）。
//!
//! 加载流程：
//! 1. 编译期内嵌 4 份契约（no_std 无法运行时读文件系统）；
//! 2. 逐份 `serde_json` 解析，失败打印文件名 + 行号 + 列号 + 原因；
//! 3. 按 `internal_path` 创建内核对象，按 `read_fields`/`write_fields` 创建条目；
//! 4. 条目 `lib_inst_id = 0` 表示占位（转换体未执行）。

use alloc::string::String;
use alloc::vec::Vec;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiField {
    pub name: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub internal_mapping: String,
    #[serde(default)]
    pub readable: bool,
    #[serde(default)]
    pub writable: bool,
}

#[derive(Debug, Deserialize)]
pub struct RateLimit {
    #[serde(default)]
    pub max_calls_per_sec: u32,
}

#[derive(Debug, Deserialize)]
pub struct ApiContract {
    pub name: String,
    #[serde(default)]
    pub version: u32,
    pub internal_path: String,
    #[serde(default)]
    pub read_fields: Vec<ApiField>,
    #[serde(default)]
    pub write_fields: Vec<ApiField>,
    #[serde(default)]
    pub rate_limit: Option<RateLimit>,
    #[serde(default)]
    pub audit: bool,
}

pub const CONTRACTS: [(&str, &str); 4] = [
    (
        "driver.manager.api",
        include_str!("../../api_definitions/driver.manager.api"),
    ),
    (
        "net.socket.api",
        include_str!("../../api_definitions/net.socket.api"),
    ),
    (
        "ui.window.api",
        include_str!("../../api_definitions/ui.window.api"),
    ),
    (
        "vm.instance.api",
        include_str!("../../api_definitions/vm.instance.api"),
    ),
];

pub struct LoadStats {
    pub contracts: usize,
    pub objects: usize,
    pub entries: usize,
    pub failed: usize,
}

/// 加载全部契约并注册到对象子系统
pub fn load_all() -> LoadStats {
    let mut stats = LoadStats {
        contracts: 0,
        objects: 0,
        entries: 0,
        failed: 0,
    };

    for (file, text) in CONTRACTS.iter() {
        let contract: ApiContract = match serde_json::from_str(text) {
            Ok(c) => c,
            Err(e) => {
                stats.failed += 1;
                crate::println!(
                    "[api] {} parse error at line {}, column {}: {}",
                    file,
                    e.line(),
                    e.column(),
                    e
                );
                continue;
            }
        };

        stats.contracts += 1;
        let obj = match crate::object::create_object(&contract.internal_path) {
            Ok(o) => {
                stats.objects += 1;
                o
            }
            Err(_) => match crate::object::lookup(&contract.internal_path) {
                Ok(o) => o,
                Err(_) => {
                    crate::println!(
                        "[api] {} internal_path {} unavailable",
                        file,
                        contract.internal_path
                    );
                    stats.failed += 1;
                    continue;
                }
            },
        };

        let mut register = |f: &ApiField| {
            if obj
                .create_entry(&f.name, crate::acl::AclMask::default_system())
                .is_ok()
            {
                // 占位条目：契约已加载，转换体未执行
                let _ = obj.write_entry(
                    &f.name,
                    b"",
                    crate::acl::AclMask::default_system(),
                );
                stats.entries += 1;
            }
        };
        for f in contract.read_fields.iter() {
            register(f);
        }
        for f in contract.write_fields.iter() {
            register(f);
        }

        crate::println!(
            "[api] contract {} v{} loaded: path={} entries={}/{} audit={}",
            contract.name,
            contract.version,
            contract.internal_path,
            contract.read_fields.len(),
            contract.write_fields.len(),
            contract.audit
        );
    }

    crate::println!(
        "[api] loaded {} contract(s), {} object(s), {} entry(s), {} failed",
        stats.contracts,
        stats.objects,
        stats.entries,
        stats.failed
    );

    stats
}
