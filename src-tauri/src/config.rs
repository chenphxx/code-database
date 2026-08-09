use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/**
 * @brief 数据库连接配置
 *
 * 优先级: 配置文件 < 环境变量覆盖
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    /** 数据库地址 */
    pub host: String,
    /** 数据库端口 */
    pub port: u16,
    /** 用户名 */
    pub user: String,
    /** 密码 */
    pub password: String,
    /** 数据库名 */
    pub database: String,
}

impl Default for DbConfig {
    /**
     * @brief 返回默认数据库配置
     *
     * @return 默认配置
     */
    fn default() -> Self {
        DbConfig {
            host: String::from("127.0.0.1"),
            port: 3306,
            user: String::from("root"),
            // 密码不在源码中提供默认值, 请在应用内「设置」或环境变量中配置
            password: String::from(""),
            database: String::from("litelearn"),
        }
    }
}

impl DbConfig {
    /**
     * @brief 使用环境变量覆盖配置
     *
     * 支持的环境变量:
     * LITELEARN_DB_HOST / LITELEARN_DB_PORT / LITELEARN_DB_USER
     * LITELEARN_DB_PASSWORD / LITELEARN_DB_NAME
     *
     * @return 无
     */
    fn apply_env(&mut self) {
        if let Ok(value) = std::env::var("LITELEARN_DB_HOST") {
            if !value.is_empty() {
                self.host = value;
            }
        }
        if let Ok(value) = std::env::var("LITELEARN_DB_PORT") {
            if let Ok(port) = value.parse::<u16>() {
                self.port = port;
            }
        }
        if let Ok(value) = std::env::var("LITELEARN_DB_USER") {
            if !value.is_empty() {
                self.user = value;
            }
        }
        if let Ok(value) = std::env::var("LITELEARN_DB_PASSWORD") {
            self.password = value;
        }
        if let Ok(value) = std::env::var("LITELEARN_DB_NAME") {
            if !value.is_empty() {
                self.database = value;
            }
        }
    }
}

/**
 * @brief 获取配置文件路径
 *
 * 配置文件存放于系统应用配置目录下的 config.json
 *
 * @param app 应用句柄
 * @return 配置文件路径
 */
fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("config.json"))
}

/**
 * @brief 加载数据库配置
 *
 * @param app 应用句柄
 * @return 数据库配置
 */
pub fn load(app: &AppHandle) -> DbConfig {
    let mut config = DbConfig::default();
    if let Ok(path) = config_path(app) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<DbConfig>(&content) {
                config = parsed;
            }
        }
    }
    config.apply_env();
    config
}

/**
 * @brief 保存数据库配置到配置文件
 *
 * @param app 应用句柄
 * @param config 数据库配置
 * @return 无
 */
pub fn save(app: &AppHandle, config: &DbConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let content = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    std::fs::write(path, content).map_err(|error| error.to_string())
}
