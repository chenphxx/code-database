use crate::config::{self, DbConfig};
use crate::db::{self, AppState};
use mysql::prelude::Queryable;
use mysql::{PooledConn, QueryResult, Text, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};

/**
 * @brief 技术栈信息
 */
#[derive(Serialize)]
pub struct StackInfo {
    /** 技术栈编号 */
    pub id: u32,
    /** 技术栈名称 */
    pub name: String,
    /** 技术栈描述 */
    pub description: String,
    /** 片段数量 */
    pub count: u64,
}

/**
 * @brief 代码片段信息
 */
#[derive(Serialize)]
pub struct Snippet {
    /** 片段编号 */
    pub id: u64,
    /** 所属技术栈编号 */
    pub stack_id: u32,
    /** 中文索引 */
    pub zh_index: String,
    /** 英文索引 */
    pub en_index: String,
    /** 代码片段 */
    pub code_snippet: String,
    /** 中文说明 */
    pub zh_comment: String,
    /** 创建时间 */
    pub created_at: String,
    /** 更新时间 */
    pub updated_at: String,
}

/**
 * @brief SQL 控制台执行结果
 */
#[derive(Serialize)]
pub struct SqlResult {
    /** 列名列表 */
    pub columns: Vec<String>,
    /** 数据行 */
    pub rows: Vec<Vec<String>>,
    /** 受影响行数 */
    pub affected: u64,
}

/**
 * @brief 备份导出结果统计
 */
#[derive(Serialize)]
pub struct ExportSummary {
    /** 技术栈数量 */
    pub stacks: usize,
    /** 片段数量 */
    pub snippets: usize,
}

/**
 * @brief 导入结果统计
 */
#[derive(Serialize, Default)]
pub struct ImportSummary {
    /** 新建技术栈数量 */
    pub stacks_created: usize,
    /** 已存在的技术栈数量 */
    pub stacks_existed: usize,
    /** 导入片段数量 */
    pub snippets_imported: usize,
    /** 跳过的片段数量(编号冲突或缺少技术栈) */
    pub snippets_skipped: usize,
}

/** @brief 导入用技术栈数据(JSON 备份) */
#[derive(Deserialize)]
struct ImportStack {
    /** 技术栈编号 */
    #[serde(default)]
    id: u32,
    /** 技术栈名称 */
    #[serde(default)]
    name: String,
    /** 技术栈描述 */
    #[serde(default)]
    description: String,
}

/** @brief 导入用片段数据(JSON 备份) */
#[derive(Deserialize)]
struct ImportSnippet {
    /** 片段编号 */
    #[serde(default)]
    id: u64,
    /** 所属技术栈编号 */
    #[serde(default)]
    stack_id: u32,
    /** 中文索引 */
    #[serde(default)]
    zh_index: String,
    /** 英文索引 */
    #[serde(default)]
    en_index: String,
    /** 代码片段 */
    #[serde(default)]
    code_snippet: String,
    /** 中文说明 */
    #[serde(default)]
    zh_comment: String,
    /** 创建时间 */
    #[serde(default)]
    created_at: Option<String>,
    /** 更新时间 */
    #[serde(default)]
    updated_at: Option<String>,
}

/** @brief JSON 备份数据 */
#[derive(Deserialize)]
struct ImportBackupData {
    /** 技术栈列表 */
    #[serde(default)]
    stacks: Vec<ImportStack>,
    /** 片段列表 */
    #[serde(default)]
    snippets: Vec<ImportSnippet>,
}

/**
 * @brief JSON 格式备份数据
 */
#[derive(Serialize)]
struct BackupData {
    /** 导出时间 */
    exported_at: String,
    /** 技术栈列表 */
    stacks: Vec<StackInfo>,
    /** 片段列表 */
    snippets: Vec<Snippet>,
}

/**
 * @brief CSV 字段转义
 *
 * 字段包含逗号、引号或换行时, 使用双引号包裹并转义内部引号
 *
 * @param value 字段值
 * @return 转义后的字段值
 */
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/**
 * @brief 生成 CSV 备份内容
 *
 * @param stacks 技术栈列表
 * @param snippets 片段列表
 * @return CSV 文本
 */
fn build_csv(stacks: &[StackInfo], snippets: &[Snippet]) -> String {
    let name_of: HashMap<u32, &str> = stacks.iter().map(|stack| (stack.id, stack.name.as_str())).collect();
    let mut lines = vec![
        "stack_name,id,zh_index,en_index,code_snippet,zh_comment,created_at,updated_at".to_string(),
    ];
    for snippet in snippets {
        let stack_name = name_of.get(&snippet.stack_id).copied().unwrap_or("");
        lines.push(format!(
            "{},{},{},{},{},{},{},{}",
            csv_escape(stack_name),
            snippet.id,
            csv_escape(&snippet.zh_index),
            csv_escape(&snippet.en_index),
            csv_escape(&snippet.code_snippet),
            csv_escape(&snippet.zh_comment),
            csv_escape(&snippet.created_at),
            csv_escape(&snippet.updated_at),
        ));
    }
    lines.join("\r\n")
}

/**
 * @brief 查询技术栈列表
 *
 * @param state 应用状态
 * @return 技术栈列表
 */
#[tauri::command]
pub async fn list_stacks(state: State<'_, AppState>) -> Result<Vec<StackInfo>, String> {
    let mut conn = db::get_conn(&state)?;
    conn.exec_map(
        "SELECT s.id, s.name, s.description, COUNT(p.id) AS count \
         FROM stacks s LEFT JOIN snippets p ON p.stack_id = s.id \
         GROUP BY s.id, s.name, s.description ORDER BY s.name",
        (),
        |(id, name, description, count)| StackInfo {
            id,
            name,
            description,
            count,
        },
    )
    .map_err(|error| error.to_string())
}

/**
 * @brief 根据技术栈与关键词检索代码片段
 *
 * 关键词为空或为 000 时查询全部数据;
 * 纯数字关键词会额外匹配片段编号;
 * 其它关键词对四个字段进行模糊匹配
 *
 * @param state 应用状态
 * @param stack_id 技术栈编号
 * @param keyword 搜索关键词
 * @return 代码片段列表
 */
#[tauri::command]
pub async fn search_snippets(
    state: State<'_, AppState>,
    stack_id: u32,
    keyword: String,
) -> Result<Vec<Snippet>, String> {
    let keyword = keyword.trim();
    let mut sql = String::from(
        "SELECT id, stack_id, zh_index, en_index, code_snippet, zh_comment, \
         DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
         DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at \
         FROM snippets WHERE stack_id = ?",
    );
    let mut params: Vec<Value> = vec![Value::from(stack_id)];

    if keyword.is_empty() || keyword == "000" {
        // 查询全部数据
        sql.push_str(" ORDER BY id");
    } else {
        // 模糊匹配四个字段, 纯数字时额外匹配编号
        let like = format!("%{}%", keyword);
        sql.push_str(
            " AND (zh_index LIKE ? OR en_index LIKE ? OR code_snippet LIKE ? OR zh_comment LIKE ?",
        );
        params.push(Value::from(like.clone()));
        params.push(Value::from(like.clone()));
        params.push(Value::from(like.clone()));
        params.push(Value::from(like));
        if let Ok(number) = keyword.parse::<u64>() {
            sql.push_str(" OR id = ?");
            params.push(Value::from(number));
        }
        sql.push_str(") ORDER BY id");
    }

    let mut conn = db::get_conn(&state)?;
    conn.exec_map(
        &sql,
        params,
        |(id, stack_id, zh_index, en_index, code_snippet, zh_comment, created_at, updated_at)| {
            Snippet {
                id,
                stack_id,
                zh_index,
                en_index,
                code_snippet,
                zh_comment,
                created_at,
                updated_at,
            }
        },
    )
    .map_err(|error| error.to_string())
}

/**
 * @brief 新建技术栈
 *
 * @param state 应用状态
 * @param name 技术栈名称
 * @param description 技术栈描述
 * @return 新技术栈编号
 */
#[tauri::command]
pub async fn create_stack(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> Result<u64, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(String::from("技术栈名称不能为空"));
    }
    if name.chars().count() > 64 {
        return Err(String::from("技术栈名称过长, 最多 64 个字符"));
    }

    let mut conn = db::get_conn(&state)?;
    conn.exec_drop(
        "INSERT INTO stacks (name, description) VALUES (?, ?)",
        (name.clone(), description.trim().to_string()),
    )
    .map_err(|error| {
        if error.to_string().contains("Duplicate entry") {
            String::from("技术栈已存在")
        } else {
            error.to_string()
        }
    })?;
    Ok(conn.last_insert_id())
}

/**
 * @brief 新增代码片段
 *
 * @param state 应用状态
 * @param stack_id 技术栈编号
 * @param zh_index 中文索引
 * @param en_index 英文索引
 * @param code_snippet 代码片段
 * @param zh_comment 中文说明
 * @return 新片段编号
 */
#[tauri::command]
pub async fn add_snippet(
    state: State<'_, AppState>,
    stack_id: u32,
    zh_index: String,
    en_index: String,
    code_snippet: String,
    zh_comment: String,
) -> Result<u64, String> {
    if code_snippet.trim().is_empty() {
        return Err(String::from("代码片段不能为空"));
    }
    if zh_index.trim().is_empty() && en_index.trim().is_empty() {
        return Err(String::from("中文索引与英文索引至少填写一项"));
    }

    let mut conn = db::get_conn(&state)?;
    conn.exec_drop(
        "INSERT INTO snippets (stack_id, zh_index, en_index, code_snippet, zh_comment) \
         VALUES (?, ?, ?, ?, ?)",
        (
            stack_id,
            zh_index.trim().to_string(),
            en_index.trim().to_string(),
            code_snippet,
            zh_comment,
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(conn.last_insert_id())
}

/**
 * @brief 更新代码片段
 *
 * @param state 应用状态
 * @param id 片段编号
 * @param zh_index 中文索引
 * @param en_index 英文索引
 * @param code_snippet 代码片段
 * @param zh_comment 中文说明
 * @return 受影响行数
 */
#[tauri::command]
pub async fn update_snippet(
    state: State<'_, AppState>,
    id: u64,
    zh_index: String,
    en_index: String,
    code_snippet: String,
    zh_comment: String,
) -> Result<u64, String> {
    let mut conn = db::get_conn(&state)?;
    conn.exec_drop(
        "UPDATE snippets SET zh_index = ?, en_index = ?, code_snippet = ?, zh_comment = ? \
         WHERE id = ?",
        (
            zh_index.trim().to_string(),
            en_index.trim().to_string(),
            code_snippet,
            zh_comment,
            id,
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(conn.affected_rows())
}

/**
 * @brief 删除代码片段
 *
 * @param state 应用状态
 * @param id 片段编号
 * @return 受影响行数
 */
#[tauri::command]
pub async fn delete_snippet(state: State<'_, AppState>, id: u64) -> Result<u64, String> {
    let mut conn = db::get_conn(&state)?;
    conn.exec_drop("DELETE FROM snippets WHERE id = ?", (id,))
        .map_err(|error| error.to_string())?;
    Ok(conn.affected_rows())
}

/**
 * @brief 导出备份数据到本地文件
 *
 * 支持 JSON 与 CSV 两种格式:
 * - JSON: 包含技术栈与片段的完整结构化数据
 * - CSV: 片段明细的扁平表格数据
 *
 * @param state 应用状态
 * @param format 导出格式, json 或 csv
 * @param path 保存文件路径
 * @return 导出结果统计
 */
#[tauri::command]
pub async fn export_backup(
    state: State<'_, AppState>,
    format: String,
    path: String,
) -> Result<ExportSummary, String> {
    let format = format.trim().to_lowercase();
    if format != "json" && format != "csv" {
        return Err(String::from("不支持的导出格式, 仅支持 json / csv"));
    }
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err(String::from("保存路径不能为空"));
    }

    let mut conn = db::get_conn(&state)?;

    // 查询全部技术栈
    let stacks: Vec<StackInfo> = conn
        .exec_map(
            "SELECT s.id, s.name, s.description, COUNT(p.id) AS count \
             FROM stacks s LEFT JOIN snippets p ON p.stack_id = s.id \
             GROUP BY s.id, s.name, s.description ORDER BY s.name",
            (),
            |(id, name, description, count)| StackInfo {
                id,
                name,
                description,
                count,
            },
        )
        .map_err(|error| error.to_string())?;

    // 查询全部片段
    let snippets: Vec<Snippet> = conn
        .exec_map(
            "SELECT p.id, p.stack_id, p.zh_index, p.en_index, p.code_snippet, p.zh_comment, \
             DATE_FORMAT(p.created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
             DATE_FORMAT(p.updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at \
             FROM snippets p ORDER BY p.stack_id, p.id",
            (),
            |(id, stack_id, zh_index, en_index, code_snippet, zh_comment, created_at, updated_at)| {
                Snippet {
                    id,
                    stack_id,
                    zh_index,
                    en_index,
                    code_snippet,
                    zh_comment,
                    created_at,
                    updated_at,
                }
            },
        )
        .map_err(|error| error.to_string())?;

    let stacks_count = stacks.len();
    let snippets_count = snippets.len();
    let content = if format == "json" {
        // 导出时间取自数据库服务器时间, 避免额外引入时间库
        let exported_at: String = conn
            .query_first("SELECT DATE_FORMAT(NOW(), '%Y-%m-%d %H:%i:%s')")
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        serde_json::to_string_pretty(&BackupData {
            exported_at,
            stacks,
            snippets,
        })
        .map_err(|error| error.to_string())?
    } else {
        build_csv(&stacks, &snippets)
    };

    std::fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(ExportSummary {
        stacks: stacks_count,
        snippets: snippets_count,
    })
}

/**
 * @brief 解析 "YYYY-MM-DD HH:MM:SS" 格式时间为数据库时间值
 *
 * 解析失败或为空时返回 NULL, 由数据库填充默认时间
 *
 * @param value 时间字符串
 * @return 数据库时间值
 */
fn parse_datetime(value: &str) -> Value {
    let value = value.trim();
    if value.is_empty() {
        return Value::NULL;
    }
    let mut parts = value.splitn(2, ' ');
    let date_part = parts.next().unwrap_or("");
    let time_part = parts.next().unwrap_or("00:00:00");
    let date_nums: Vec<u32> = date_part
        .split('-')
        .filter_map(|part| part.parse().ok())
        .collect();
    let time_nums: Vec<u32> = time_part
        .split(':')
        .filter_map(|part| part.parse().ok())
        .collect();
    if date_nums.len() != 3 {
        return Value::NULL;
    }
    Value::Date(
        date_nums[0] as u16,
        date_nums[1] as u8,
        date_nums[2] as u8,
        time_nums.first().copied().unwrap_or(0) as u8,
        time_nums.get(1).copied().unwrap_or(0) as u8,
        time_nums.get(2).copied().unwrap_or(0) as u8,
        0,
    )
}

/**
 * @brief 解析 CSV 单行, 支持引号包裹与双引号转义
 *
 * @param line CSV 行文本
 * @return 字段列表
 */
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

/**
 * @brief 获取或创建技术栈, 返回数据库中的技术栈编号
 *
 * @param conn 数据库连接
 * @param name 技术栈名称
 * @param stack_id_map 名称到编号的缓存映射
 * @param summary 导入统计
 * @return 技术栈编号
 */
fn ensure_stack(
    conn: &mut PooledConn,
    name: &str,
    description: &str,
    stack_id_map: &mut HashMap<String, u32>,
    summary: &mut ImportSummary,
) -> Result<u32, String> {
    if let Some(id) = stack_id_map.get(name) {
        return Ok(*id);
    }
    if let Some(id) = conn
        .exec_first("SELECT id FROM stacks WHERE name = ?", (name,))
        .map_err(|error| error.to_string())?
    {
        stack_id_map.insert(name.to_string(), id);
        summary.stacks_existed += 1;
        return Ok(id);
    }
    conn.exec_drop(
        "INSERT INTO stacks (name, description) VALUES (?, ?)",
        (name, description),
    )
    .map_err(|error| error.to_string())?;
    let id = conn.last_insert_id() as u32;
    stack_id_map.insert(name.to_string(), id);
    summary.stacks_created += 1;
    Ok(id)
}

/**
 * @brief 导入 JSON 格式备份数据
 *
 * @param conn 数据库连接
 * @param content 文件内容
 * @return 导入统计
 */
fn import_json(conn: &mut PooledConn, content: &str) -> Result<ImportSummary, String> {
    let data: ImportBackupData =
        serde_json::from_str(content).map_err(|error| format!("JSON 解析失败: {}", error))?;
    let mut summary = ImportSummary::default();

    // 建立导出文件中的技术栈编号 -> 名称映射
    let stack_names: HashMap<u32, String> = data
        .stacks
        .iter()
        .map(|stack| (stack.id, stack.name.clone()))
        .collect();
    let mut stack_id_map: HashMap<String, u32> = HashMap::new();

    // 先确保所有技术栈存在
    for stack in &data.stacks {
        let name = stack.name.trim();
        if name.is_empty() {
            continue;
        }
        let _ = ensure_stack(
            conn,
            name,
            stack.description.trim(),
            &mut stack_id_map,
            &mut summary,
        )?;
    }

    // 再导入片段
    for snippet in &data.snippets {
        let stack_name = stack_names.get(&snippet.stack_id).cloned().unwrap_or_default();
        if stack_name.trim().is_empty() {
            summary.snippets_skipped += 1;
            continue;
        }
        let db_stack_id = ensure_stack(
            conn,
            stack_name.trim(),
            "",
            &mut stack_id_map,
            &mut summary,
        )?;

        // 编号已存在时跳过, 避免重复导入
        if snippet.id != 0 {
            let exists: u64 = conn
                .exec_first("SELECT COUNT(*) FROM snippets WHERE id = ?", (snippet.id,))
                .map_err(|error| error.to_string())?
                .unwrap_or(0);
            if exists > 0 {
                summary.snippets_skipped += 1;
                continue;
            }
        }

        conn.exec_drop(
            "INSERT INTO snippets \
             (id, stack_id, zh_index, en_index, code_snippet, zh_comment, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                if snippet.id == 0 {
                    None
                } else {
                    Some(snippet.id)
                },
                db_stack_id,
                snippet.zh_index.trim().to_string(),
                snippet.en_index.trim().to_string(),
                snippet.code_snippet.clone(),
                snippet.zh_comment.clone(),
                parse_datetime(snippet.created_at.as_deref().unwrap_or("")),
                parse_datetime(snippet.updated_at.as_deref().unwrap_or("")),
            ),
        )
        .map_err(|error| error.to_string())?;
        summary.snippets_imported += 1;
    }
    Ok(summary)
}

/**
 * @brief 导入 CSV 格式备份数据
 *
 * @param conn 数据库连接
 * @param content 文件内容
 * @return 导入统计
 */
fn import_csv(conn: &mut PooledConn, content: &str) -> Result<ImportSummary, String> {
    let mut summary = ImportSummary::default();
    let mut stack_id_map: HashMap<String, u32> = HashMap::new();
    let mut lines = content.lines();

    // 跳过表头
    let _header = lines.next();
    for (index, line) in lines.enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        if fields.len() < 6 {
            return Err(format!("CSV 第 {} 行字段数不足", index + 2));
        }
        let stack_name = fields[0].trim().to_string();
        let snippet_id: u64 = fields[1].trim().parse().unwrap_or(0);
        if stack_name.is_empty() {
            summary.snippets_skipped += 1;
            continue;
        }
        let db_stack_id = ensure_stack(conn, &stack_name, "", &mut stack_id_map, &mut summary)?;

        if snippet_id != 0 {
            let exists: u64 = conn
                .exec_first("SELECT COUNT(*) FROM snippets WHERE id = ?", (snippet_id,))
                .map_err(|error| error.to_string())?
                .unwrap_or(0);
            if exists > 0 {
                summary.snippets_skipped += 1;
                continue;
            }
        }

        conn.exec_drop(
            "INSERT INTO snippets \
             (id, stack_id, zh_index, en_index, code_snippet, zh_comment, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                if snippet_id == 0 {
                    None
                } else {
                    Some(snippet_id)
                },
                db_stack_id,
                fields[2].trim().to_string(),
                fields[3].trim().to_string(),
                fields[4].clone(),
                fields[5].clone(),
                parse_datetime(fields.get(6).map(String::as_str).unwrap_or("")),
                parse_datetime(fields.get(7).map(String::as_str).unwrap_or("")),
            ),
        )
        .map_err(|error| error.to_string())?;
        summary.snippets_imported += 1;
    }
    Ok(summary)
}

/**
 * @brief 从备份文件导入数据
 *
 * 支持 JSON 与 CSV 两种格式, 采用合并导入方式:
 * - 技术栈按名称匹配, 不存在时自动创建
 * - 片段编号已存在时跳过, 不覆盖已有数据
 * - 时间字段可还原为备份时的值
 *
 * @param state 应用状态
 * @param format 导入格式, json 或 csv
 * @param path 备份文件路径
 * @return 导入结果统计
 */
#[tauri::command]
pub async fn import_backup(
    state: State<'_, AppState>,
    format: String,
    path: String,
) -> Result<ImportSummary, String> {
    let format = format.trim().to_lowercase();
    if format != "json" && format != "csv" {
        return Err(String::from("不支持的导入格式, 仅支持 json / csv"));
    }
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err(String::from("文件路径不能为空"));
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取文件失败: {}", error))?;
    let mut conn = db::get_conn(&state)?;
    if format == "json" {
        import_json(&mut conn, &content)
    } else {
        import_csv(&mut conn, &content)
    }
}

/**
 * @brief 获取当前数据库配置
 *
 * @param state 应用状态
 * @return 数据库配置
 */
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<DbConfig, String> {
    state
        .config
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| String::from("配置状态异常"))
}

/**
 * @brief 校验数据库配置合法性
 *
 * @param config 数据库配置
 * @return 无
 */
fn validate_config(config: &DbConfig) -> Result<(), String> {
    if config.host.trim().is_empty() {
        return Err(String::from("数据库地址不能为空"));
    }
    if config.port == 0 {
        return Err(String::from("端口号无效"));
    }
    if config.user.trim().is_empty() {
        return Err(String::from("用户名不能为空"));
    }
    let database = config.database.trim();
    if database.is_empty() {
        return Err(String::from("数据库名不能为空"));
    }
    if database.contains('`') || database.contains(';') || database.contains('"') || database.contains('\'') {
        return Err(String::from("数据库名包含非法字符"));
    }
    Ok(())
}

/**
 * @brief 测试数据库连接
 *
 * @param state 应用状态
 * @param config 数据库配置
 * @return MySQL 版本号
 */
#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    config: DbConfig,
) -> Result<String, String> {
    validate_config(&config)?;
    let _ = &state;
    let pool = db::create_pool(&config)?;
    let mut conn = pool.get_conn().map_err(|error| error.to_string())?;
    let version: String = conn
        .query_first("SELECT VERSION()")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| String::from("未知版本"));
    Ok(version)
}

/**
 * @brief 保存数据库配置并重新连接
 *
 * @param app 应用句柄
 * @param state 应用状态
 * @param config 数据库配置
 * @return 无
 */
#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: DbConfig,
) -> Result<(), String> {
    validate_config(&config)?;
    // 先测试连接, 成功后才允许保存
    let _pool = db::create_pool(&config)?;
    config::save(&app, &config)?;
    *state
        .config
        .lock()
        .map_err(|_| String::from("配置状态异常"))? = config;
    state.reset_pool();
    Ok(())
}

/**
 * @brief 执行 SQL 语句
 *
 * @param state 应用状态
 * @param sql SQL 语句
 * @return 执行结果
 */
#[tauri::command]
pub async fn execute_sql(state: State<'_, AppState>, sql: String) -> Result<SqlResult, String> {
    let sql = sql.trim().to_string();
    if sql.is_empty() {
        return Err(String::from("SQL 语句不能为空"));
    }

    let mut conn = db::get_conn(&state)?;
    let mut result: QueryResult<Text> = conn.query_iter(&sql).map_err(|error| error.to_string())?;
    let columns: Vec<String> = result
        .columns()
        .as_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in result.by_ref() {
        let row = row.map_err(|error| error.to_string())?;
        let values: Vec<String> = row
            .unwrap()
            .iter()
            .map(|value| value.as_sql(false))
            .collect();
        rows.push(values);
    }
    let affected = result.affected_rows();
    Ok(SqlResult {
        columns,
        rows,
        affected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;

    /**
     * @brief 构造测试数据库配置
     *
     * @return 测试数据库配置
     */
    fn test_config() -> DbConfig {
        let password = std::env::var("LITELEARN_DB_PASSWORD").unwrap_or_default();
        DbConfig {
            host: String::from("127.0.0.1"),
            port: 3306,
            user: String::from("root"),
            password,
            database: String::from("litelearn_test"),
        }
    }

    /**
     * @brief 备份导入往返测试
     *
     * 需要本机 MySQL 服务, 通过环境变量 LITELEARN_RUN_DB_TEST=1 触发;
     * 测试在独立的 litelearn_test 数据库中进行, 结束后自动删除
     */
    #[test]
    fn test_import_round_trip() {
        if std::env::var("LITELEARN_RUN_DB_TEST").is_err()
            || std::env::var("LITELEARN_DB_PASSWORD").is_err()
        {
            return;
        }
        let config = test_config();
        let pool = create_pool(&config).expect("连接测试数据库失败");
        let mut conn = pool.get_conn().expect("获取连接失败");

        // 清空测试数据
        conn.query_drop("DELETE FROM snippets").expect("清空片段失败");
        conn.query_drop("DELETE FROM stacks").expect("清空技术栈失败");

        // JSON 导入: 与导出格式保持一致
        let json = r#"{
            "exported_at": "2026-08-11 10:00:00",
            "stacks": [
                { "id": 1, "name": "TestStack", "description": "测试技术栈" }
            ],
            "snippets": [
                {
                    "id": 100,
                    "stack_id": 1,
                    "zh_index": "测试索引",
                    "en_index": "test",
                    "code_snippet": "println!(\"hello\");",
                    "zh_comment": "中文说明",
                    "created_at": "2024-09-20 12:34:56",
                    "updated_at": "2024-09-20 12:34:56"
                }
            ]
        }"#;
        let summary = import_json(&mut conn, json).expect("JSON 导入失败");
        assert_eq!(summary.stacks_created, 1);
        assert_eq!(summary.snippets_imported, 1);

        // 重复导入应全部跳过(幂等)
        let summary2 = import_json(&mut conn, json).expect("JSON 重复导入失败");
        assert_eq!(summary2.stacks_existed, 1);
        assert_eq!(summary2.snippets_skipped, 1);
        assert_eq!(summary2.snippets_imported, 0);

        // 验证描述与时间已还原
        let description: String = conn
            .query_first("SELECT description FROM stacks WHERE name = 'TestStack'")
            .expect("查询描述失败")
            .expect("技术栈不存在");
        assert_eq!(description, "测试技术栈");
        let created: String = conn
            .query_first(
                "SELECT DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') FROM snippets WHERE id = 100",
            )
            .expect("查询时间失败")
            .expect("片段不存在");
        assert_eq!(created, "2024-09-20 12:34:56");

        // CSV 导入: 验证引号包裹与逗号字段
        let csv = "stack_name,id,zh_index,en_index,code_snippet,zh_comment,created_at,updated_at\n\
                   TestStackCsv,200,中文,en2,\"code, with comma\",说明,2024-09-20 12:34:56,2024-09-20 12:34:56\n";
        let summary_csv = import_csv(&mut conn, csv).expect("CSV 导入失败");
        assert_eq!(summary_csv.stacks_created, 1);
        assert_eq!(summary_csv.snippets_imported, 1);
        let code: String = conn
            .query_first("SELECT code_snippet FROM snippets WHERE id = 200")
            .expect("查询代码失败")
            .expect("片段不存在");
        assert_eq!(code, "code, with comma");

        // 清理测试数据库
        drop(conn);
        drop(pool);
        let cleanup_config = DbConfig {
            database: String::from("litelearn"),
            ..config
        };
        let cleanup_pool = create_pool(&cleanup_config).expect("连接清理数据库失败");
        let mut cleanup_conn = cleanup_pool.get_conn().expect("获取清理连接失败");
        cleanup_conn
            .query_drop("DROP DATABASE IF EXISTS litelearn_test")
            .expect("删除测试数据库失败");
    }
}
