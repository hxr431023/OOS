//! .api 契约解析器（D6 修订版）
//!
//! 事实源：各文档附赠的 `.api` 片段（见 `specs/docs/acl.md` 等）为准；
//! `specs/docs/.api.md` 已按片段修订（条目读写判定、跨行大括号、重复字段）。
//! `api_definitions/*.api` 现为 JSON，与片段 DSL 不符，仅作兼容路径并打印告警。

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

// ---------------- JSON 兼容路径 ----------------

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

// ---------------- DSL（片段格式） ----------------

#[derive(Debug, Default)]
pub struct DslEntry {
    pub object: String,
    pub name: String,
    pub code: String,
    pub internal_path: String,
}

#[derive(Debug, Default)]
pub struct DslContract {
    pub version: String,
    pub target: String,
    pub env: String,
    pub exec: String,
    pub internal_loc: String,
    pub min_os_version: String,
    pub global_acl: String,
    pub global_limit: String,
    pub objects: Vec<(String, String)>,
    pub entries: Vec<DslEntry>,
    pub events: Vec<(String, String)>,
    pub limit_overrides: Vec<(String, String)>,
    pub acl_overrides: Vec<(String, String)>,
}

enum Section {
    Header,
    Event,
    Limit,
    Acl,
}

/// 解析片段格式全文；错误包含文件名 + 行号 + 原因
pub fn parse_dsl(text: &str, file: &str) -> Result<DslContract, String> {
    let mut c = DslContract::default();
    let mut section = Section::Header;
    let mut pending: Option<DslEntry> = None;
    let mut depth: i32 = 0;

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let lineno = idx + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // 多行大括号累积
        if let Some(e) = pending.as_mut() {
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if let Some(pos) = line.rfind('}') {
                let head = line[..pos].trim();
                if !head.is_empty() {
                    e.code.push_str(head);
                    e.code.push('\n');
                }
                e.internal_path = line[pos + 1..].trim().to_string();
            } else {
                e.code.push_str(line);
                e.code.push('\n');
            }
            if depth <= 0 {
                if e.internal_path.is_empty() {
                    return Err(alloc::format!(
                        "{}:{}: entry `{}` missing internal path",
                        file,
                        lineno,
                        e.name
                    ));
                }
                c.entries.push(pending.take().unwrap());
                depth = 0;
            }
            continue;
        }

        if line == "EVENT:" {
            section = Section::Event;
            continue;
        }
        if line == "LIMIT_OVERRIDE:" {
            section = Section::Limit;
            continue;
        }
        if line == "ACL_OVERRIDE:" {
            section = Section::Acl;
            continue;
        }

        match section {
            Section::Event => {
                if let Some((sig, internal)) = line.split_once('｜') {
                    c.events
                        .push((sig.trim().to_string(), internal.trim().to_string()));
                } else {
                    return Err(alloc::format!(
                        "{}:{}: EVENT line missing `｜`",
                        file,
                        lineno
                    ));
                }
                continue;
            }
            Section::Limit | Section::Acl => {
                if let Some((k, v)) = line.split_once(':') {
                    let pair = (k.trim().to_string(), v.trim().to_string());
                    if matches!(section, Section::Limit) {
                        c.limit_overrides.push(pair);
                    } else {
                        c.acl_overrides.push(pair);
                    }
                } else {
                    return Err(alloc::format!(
                        "{}:{}: override line missing `:`",
                        file,
                        lineno
                    ));
                }
                continue;
            }
            Section::Header => {}
        }

        if let Some(rest) = line.strip_prefix("object ") {
            let mut it = rest.split_whitespace();
            let dir = it.next().unwrap_or("").to_string();
            let name = it.next().unwrap_or("").to_string();
            if dir.is_empty() || name.is_empty() {
                return Err(alloc::format!(
                    "{}:{}: object line needs <dir> and <name>",
                    file,
                    lineno
                ));
            }
            c.objects.push((dir, name));
            continue;
        }

        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim().to_string();
            match key {
                "version" => c.version = val,
                "target" => c.target = val,
                "env" => c.env = val,
                "exec" => c.exec = val,
                "internal_loc" => c.internal_loc = val,
                "min_os_version" => c.min_os_version = val,
                "global_acl" => c.global_acl = val,
                "global_limit" => c.global_limit = val,
                _ => {}
            }
            continue;
        }

        // 条目：<对象>: <条目名> { ... } <内部路径>
        if let Some((obj, rest)) = line.split_once(':') {
            let obj = obj.trim();
            let rest = rest.trim();
            if let Some(brace) = rest.find('{') {
                let name = rest[..brace].trim().to_string();
                let after_open = &rest[brace + 1..];
                if let Some(close) = after_open.find('}') {
                    let code = after_open[..close].trim().to_string();
                    let internal = after_open[close + 1..].trim().to_string();
                    if internal.is_empty() {
                        return Err(alloc::format!(
                            "{}:{}: entry `{}` missing internal path",
                            file,
                            lineno,
                            name
                        ));
                    }
                    c.entries.push(DslEntry {
                        object: obj.to_string(),
                        name,
                        code,
                        internal_path: internal,
                    });
                } else {
                    depth = 1;
                    pending = Some(DslEntry {
                        object: obj.to_string(),
                        name,
                        code: alloc::format!("{}\n", after_open),
                        internal_path: String::new(),
                    });
                }
                continue;
            }
        }

        return Err(alloc::format!(
            "{}:{}: unrecognized line: {}",
            file,
            lineno,
            line
        ));
    }

    if pending.is_some() {
        return Err(alloc::format!("{}: unterminated `{{` block", file));
    }
    Ok(c)
}

/// 片段读写判定（已写入 .api.md）：
/// 含 set_/write/register/raise 为可写，其余（return / 空透传）为只读。
fn infer_acl(code: &str) -> crate::acl::AclMask {
    let w = code.contains("set_")
        || code.contains("write")
        || code.contains("register")
        || code.contains("raise");
    if w {
        crate::acl::AclMask::ENTRY_WRITE
    } else {
        crate::acl::AclMask::ENTRY_READ
    }
}

pub const CONTRACTS: [(&str, &str); 7] = [
    (
        "driver.manager.api",
        include_str!("../../../api_definitions/driver.manager.api"),
    ),
    (
        "net.socket.api",
        include_str!("../../../api_definitions/net.socket.api"),
    ),
    (
        "ui.window.api",
        include_str!("../../../api_definitions/ui.window.api"),
    ),
    (
        "vm.instance.api",
        include_str!("../../../api_definitions/vm.instance.api"),
    ),
    ("boot.api", include_str!("../../../system/api/boot.api")),
    ("service.api", include_str!("../../../system/api/service.api")),
    ("kernel.api", include_str!("../../../system/api/kernel.api")),
];

pub struct LoadStats {
    pub contracts: usize,
    pub objects: usize,
    pub entries: usize,
    pub failed: usize,
    pub dsl: usize,
    pub json: usize,
}

fn ensure_object(path: &str) -> Option<alloc::sync::Arc<crate::object::KObject>> {
    match crate::object::create_object(path) {
        Ok(o) => Some(o),
        Err(_) => crate::object::lookup(path).ok(),
    }
}

pub fn load_all() -> LoadStats {
    let mut stats = LoadStats {
        contracts: 0,
        objects: 0,
        entries: 0,
        failed: 0,
        dsl: 0,
        json: 0,
    };

    for (file, text) in CONTRACTS.iter() {
        let trimmed = text.trim_start();

        if trimmed.starts_with('{') {
            let contract: ApiContract = match serde_json::from_str(text) {
                Ok(c) => c,
                Err(e) => {
                    stats.failed += 1;
                    crate::println!(
                        "[api] {}: JSON parse error at line {}, column {}: {}",
                        file,
                        e.line(),
                        e.column(),
                        e
                    );
                    continue;
                }
            };
            stats.json += 1;
            stats.contracts += 1;

            let obj = match ensure_object(&contract.internal_path) {
                Some(o) => {
                    stats.objects += 1;
                    o
                }
                None => {
                    crate::println!(
                        "[api] {}: object {} unavailable",
                        file,
                        contract.internal_path
                    );
                    stats.failed += 1;
                    continue;
                }
            };

            let mut register = |f: &ApiField| {
                let acl = match (f.readable, f.writable) {
                    (true, true) => {
                        crate::acl::AclMask::ENTRY_READ | crate::acl::AclMask::ENTRY_WRITE
                    }
                    (true, false) => crate::acl::AclMask::ENTRY_READ,
                    (false, true) => crate::acl::AclMask::ENTRY_WRITE,
                    (false, false) => return,
                };
                if obj
                    .create_entry_with_acl(
                        &f.name,
                        crate::acl::AclMask::default_system(),
                        acl,
                        &f.internal_mapping,
                    )
                    .is_ok()
                {
                    stats.entries += 1;
                }
            };
            for f in contract.read_fields.iter() {
                register(f);
            }
            for f in contract.write_fields.iter() {
                register(f);
            }

            let rate = contract
                .rate_limit
                .as_ref()
                .map(|r| r.max_calls_per_sec)
                .unwrap_or(0);
            crate::println!(
                "[api] {}: rate_limit={}/s audit={} (not enforced)",
                contract.name,
                rate,
                contract.audit
            );
            crate::println!(
                "[api] contract {} v{} loaded (JSON file, spec is DSL fragment): path={} entries={}/{}",
                contract.name,
                contract.version,
                contract.internal_path,
                contract.read_fields.len(),
                contract.write_fields.len()
            );
            continue;
        }

        let contract = match parse_dsl(text, file) {
            Ok(c) => c,
            Err(msg) => {
                stats.failed += 1;
                crate::println!("[api] {}: parse error: {}", file, msg);
                continue;
            }
        };
        stats.dsl += 1;
        stats.contracts += 1;

        let mut obj = None;
        if let Some((dir, _name)) = contract.objects.first() {
            obj = ensure_object(dir);
        }
        let obj = match obj {
            Some(o) => {
                stats.objects += 1;
                o
            }
            None => {
                crate::println!("[api] {}: no object declared", file);
                stats.failed += 1;
                continue;
            }
        };

        for e in contract.entries.iter() {
            let acl = infer_acl(&e.code);
            if obj
                .create_entry_with_acl(
                    &e.name,
                    crate::acl::AclMask::default_system(),
                    acl,
                    &e.internal_path,
                )
                .is_ok()
            {
                stats.entries += 1;
            }
        }

        crate::println!(
            "[api] {}: env={} global_acl={} global_limit={} events={} (not enforced)",
            contract.target,
            contract.env,
            contract.global_acl,
            contract.global_limit,
            contract.events.len()
        );
        crate::println!(
            "[api] contract {} v{} loaded (DSL): object={} entries={}",
            contract.target,
            contract.version,
            contract
                .objects
                .first()
                .map(|(d, _)| d.as_str())
                .unwrap_or(""),
            contract.entries.len()
        );
    }

    crate::println!(
        "[api] loaded {} contract(s), {} object(s), {} entry(s); dsl={} json={} failed={}",
        stats.contracts,
        stats.objects,
        stats.entries,
        stats.dsl,
        stats.json,
        stats.failed
    );

    stats
}
