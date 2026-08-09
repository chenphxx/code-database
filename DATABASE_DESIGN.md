# LiteLearn 数据库设计文档

> 本文档与 `README.md` 同目录存放, 用于描述 LiteLearn 重新开发后的数据库设计。

## 1. 概述

- 数据库类型: MySQL 9.x
- 数据库名称: `litelearn`
- 字符集: `utf8mb4` / `utf8mb4_unicode_ci`(支持中文与 emoji)
- 存储引擎: InnoDB(支持外键与事务)
- 作用: 存储代码速查知识库, 包括技术栈与其下的代码片段

## 2. 设计原则

旧版项目采用"一个技术栈一张表"的设计, 每个技术栈对应 SQLite 中的一张表, 存在以下问题:

- 表结构无法统一, 部分表缺少主键与自增约束
- 新增技术栈需要动态建表, 难以维护
- 无法跨技术栈检索与统计

新版采用 **`stacks`(技术栈) 1:N `snippets`(代码片段)** 的两张表设计, 解决了上述问题。

## 3. 表结构

### 3.1 技术栈表 `stacks`

```sql
CREATE TABLE IF NOT EXISTS stacks (
    id          INT UNSIGNED  NOT NULL AUTO_INCREMENT COMMENT '技术栈编号',
    name        VARCHAR(64)   NOT NULL                COMMENT '技术栈名称, 唯一',
    description VARCHAR(255)  NOT NULL DEFAULT ''     COMMENT '技术栈描述',
    created_at  DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (id),
    UNIQUE KEY uk_stacks_name (name)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci
  COMMENT = '技术栈表';
```

### 3.2 代码片段表 `snippets`

```sql
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
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci
  COMMENT = '代码片段表';
```

## 4. 字段说明

| 表 | 字段 | 说明 |
| --- | --- | --- |
| stacks | id | 自增主键 |
| stacks | name | 技术栈名称, 如 `C` / `CPP` / `Rust` |
| stacks | description | 技术栈描述, 可留空 |
| stacks | created_at | 创建时间 |
| snippets | id | 自增主键, 迁移时沿用旧版 `number_index` 的数值 |
| snippets | stack_id | 所属技术栈, 级联删除 |
| snippets | zh_index | 中文索引, 用于检索 |
| snippets | en_index | 英文索引, 用于检索 |
| snippets | code_snippet | 代码片段正文 |
| snippets | zh_comment | 中文说明/注释 |
| snippets | created_at / updated_at | 创建/更新时间, 更新时间自动维护 |

## 5. 检索设计

- 输入 `000` 或空字符串: 查询当前技术栈下全部数据
- 输入纯数字(非 `000`): 按片段编号 `id` 精确匹配, 兼容旧版按 `number_index` 查询的习惯
- 输入其它关键词: 对 `zh_index`、`en_index`、`code_snippet`、`zh_comment` 进行 `LIKE '%关键词%'` 模糊匹配
- 不区分中英文输入, 两种索引字段同时参与匹配

## 6. 备份与导入导出

- 应用内「导出备份」功能可导出全部数据
- JSON 格式: 包含技术栈与片段的完整结构化数据, 便于长期存档与再次导入
- CSV 格式: 片段明细的扁平表格数据, 便于用表格软件打开查看
- 导出文件由 Rust 后端生成, 支持自定义保存位置
- 应用内「导入备份」可将导出的 JSON / CSV 文件重新导入
- 导入采用合并方式: 技术栈按名称匹配并自动创建, 片段编号已存在时跳过, 不覆盖已有数据
- 导入时可还原备份时的创建/更新时间

## 7. 后续可扩展

- 标签系统: 可新增 `tags` 表, 与 `snippets` 多对多关联
- 收藏/复习: 可在 `snippets` 增加 `is_favorite`、`review_count` 等字段
- 全文检索: 数据量增大后可引入 MySQL 全文索引或外部检索引擎
