use crate::config::DbConfig;
use mysql::prelude::Queryable;
use mysql::{Opts, OptsBuilder, Pool, PooledConn};
use std::sync::Mutex;

/** 技术栈表建表语句 */
const CREATE_STACKS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS stacks (
    id          INT UNSIGNED  NOT NULL AUTO_INCREMENT COMMENT '技术栈编号',
    name        VARCHAR(64)   NOT NULL                COMMENT '技术栈名称, 唯一',
    description VARCHAR(255)  NOT NULL DEFAULT ''     COMMENT '技术栈描述',
    created_at  DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (id),
    UNIQUE KEY uk_stacks_name (name)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci COMMENT = '技术栈表'"#;

/** 代码片段表建表语句 */
const CREATE_SNIPPETS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS snippets (
    id           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT COMMENT '片段编号, 兼容旧版 number_index',
    stack_id     INT UNSIGNED    NOT NULL                COMMENT '所属技术栈, 外键',
    zh_index     VARCHAR(128)    NOT NULL DEFAULT ''     COMMENT '中文索引',
    en_index     VARCHAR(128)    NOT NULL DEFAULT ''     COMMENT '英文索引',
    code_snippet MEDIUMTEXT      NOT NULL                COMMENT '代码片段',
    zh_comment   MEDIUMTEXT      NOT NULL                COMMENT '中文说明',
    created_at   DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    updated_at   DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP
                                 ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    PRIMARY KEY (id),
    KEY idx_snippets_stack_zh (stack_id, zh_index),
    KEY idx_snippets_stack_en (stack_id, en_index),
    CONSTRAINT fk_snippets_stack
        FOREIGN KEY (stack_id) REFERENCES stacks (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci COMMENT = '代码片段表'"#;

/**
 * @brief 应用全局状态
 */
pub struct AppState {
    /** 数据库连接配置 */
    pub config: Mutex<DbConfig>,
    /** 数据库连接池, 懒加载 */
    pub pool: Mutex<Option<Pool>>,
}

impl AppState {
    /**
     * @brief 创建应用状态
     *
     * @param config 数据库连接配置
     * @return 应用状态
     */
    pub fn new(config: DbConfig) -> Self {
        AppState {
            config: Mutex::new(config),
            pool: Mutex::new(None),
        }
    }

    /**
     * @brief 获取数据库连接池, 不存在时按当前配置创建
     *
     * @return 数据库连接池
     */
    pub fn get_pool(&self) -> Result<Pool, String> {
        let mut guard = self.pool.lock().map_err(|_| String::from("连接池状态异常"))?;
        if let Some(pool) = guard.as_ref() {
            return Ok(pool.clone());
        }
        let config = self
            .config
            .lock()
            .map_err(|_| String::from("配置状态异常"))?
            .clone();
        let pool = create_pool(&config)?;
        *guard = Some(pool.clone());
        Ok(pool)
    }

    /**
     * @brief 重置连接池, 使下次操作按新配置重新连接
     *
     * @return 无
     */
    pub fn reset_pool(&self) {
        if let Ok(mut guard) = self.pool.lock() {
            *guard = None;
        }
    }
}

/**
 * @brief 根据配置构建连接参数
 *
 * @param config 数据库配置
 * @param with_db 是否指定数据库
 * @return 连接参数
 */
fn build_opts(config: &DbConfig, with_db: bool) -> Opts {
    let mut builder = OptsBuilder::new()
        .ip_or_hostname(Some(config.host.clone()))
        .tcp_port(config.port)
        .user(Some(config.user.clone()))
        .pass(Some(config.password.clone()));
    if with_db {
        builder = builder.db_name(Some(config.database.clone()));
    }
    builder.into()
}

/**
 * @brief 根据配置创建连接池, 并初始化数据库与表结构
 *
 * @param config 数据库配置
 * @return 数据库连接池
 */
pub fn create_pool(config: &DbConfig) -> Result<Pool, String> {
    // 1. 先连接服务器(不指定数据库), 确保目标数据库存在
    let bootstrap_pool = Pool::new(build_opts(config, false)).map_err(|error| error.to_string())?;
    let mut bootstrap_conn = bootstrap_pool
        .get_conn()
        .map_err(|error| error.to_string())?;
    bootstrap_conn
        .query_drop(format!(
            "CREATE DATABASE IF NOT EXISTS `{}` DEFAULT CHARACTER SET utf8mb4 DEFAULT COLLATE utf8mb4_unicode_ci",
            config.database
        ))
        .map_err(|error| error.to_string())?;
    drop(bootstrap_conn);

    // 2. 连接目标数据库, 初始化表结构
    let pool = Pool::new(build_opts(config, true)).map_err(|error| error.to_string())?;
    let mut conn = pool.get_conn().map_err(|error| error.to_string())?;
    conn.query_drop(CREATE_STACKS_SQL)
        .map_err(|error| error.to_string())?;
    conn.query_drop(CREATE_SNIPPETS_SQL)
        .map_err(|error| error.to_string())?;
    Ok(pool)
}

/**
 * @brief 获取一条数据库连接
 *
 * @param state 应用状态
 * @return 数据库连接
 */
pub fn get_conn(state: &AppState) -> Result<PooledConn, String> {
    let pool = state.get_pool()?;
    pool.get_conn().map_err(|error| error.to_string())
}
