# litelearn

## 项目简介

- 按技术栈组织代码片段, 支持中文/英文索引检索
- 支持模糊搜索, 输入 `000` 查看当前技术栈全部数据
- 支持新增、修改、删除片段, 支持新建技术栈
- 支持备份导出与无缝重新导入(JSON / CSV)
- 内置 SQL 控制台, 可手动执行 SQL 语句
- 数据库连接信息可在应用内配置

## 技术栈

| 层    | 技术                                |
| ---- | --------------------------------- |
| 前端   | HTML / CSS / 原生 JavaScript + Vite |
| 桌面框架 | Tauri 2.x(Rust)                   |
| 数据库  | MySQL 9.x                         |
| 构建   | npm + Cargo                       |

## 项目架构

```text
litelearn/
├── index.html             # 前端入口页面
├── src/                   # 前端源码
│   ├── main.js            # 前端逻辑, 通过 Tauri invoke 调用后端命令
│   └── styles.css         # 样式
├── src-tauri/             # Rust 后端
│   ├── src/
│   │   ├── main.rs        # 程序入口
│   │   ├── lib.rs         # Tauri 应用装配与命令注册
│   │   ├── config.rs      # 数据库连接配置读写
│   │   ├── db.rs          # MySQL 连接池与表结构初始化
│   │   └── commands.rs    # Tauri 命令(技术栈/片段/设置/SQL)
│   ├── tauri.conf.json    # Tauri 配置
│   └── capabilities/      # 权限配置
├── DATABASE_DESIGN.md     # 数据库设计文档
└── CHANGELOG.md           # 更新日志
```

前后端通过 Tauri Command 通信, 前端不直接接触数据库; 
数据库访问集中在 `db.rs` 与 `commands.rs`, 连接配置由 `config.rs` 管理 

## 环境要求

- Node.js 18+
- Rust stable(建议 1.97+)
- MySQL 9.x(本地或远程均可)

## 快速开始

### 1. 准备数据库

首次运行前需创建数据库与表结构: 参照 `DATABASE_DESIGN.md` 中的建表语句,
在 MySQL 中创建 `litelearn` 数据库及 `stacks`、`snippets` 两张表。
应用首次连接时会自动补齐缺失的表结构。

### 2. 安装前端依赖

```bash
npm install
```

### 3. 开发运行

```bash
npm run tauri dev
```

### 4. 构建打包

```bash
npm run tauri build
```

安装包输出于 `src-tauri/target/release/bundle/`。

## 数据库配置

- 默认连接: `127.0.0.1:3306`, 用户 `root`, 数据库 `litelearn`;
  密码不提供默认值, 请通过应用内「设置」保存, 或使用环境变量 `LITELEARN_DB_PASSWORD` 指定

- 应用内点击「设置」可修改连接信息, 保存后自动重连; 配置保存在
  `%APPDATA%/com.litelearn.app/config.json`

- 支持环境变量覆盖配置(优先级高于配置文件):
  
  | 环境变量                    | 说明    |
  | ----------------------- | ----- |
  | `LITELEARN_DB_HOST`     | 数据库地址 |
  | `LITELEARN_DB_PORT`     | 端口    |
  | `LITELEARN_DB_USER`     | 用户名   |
  | `LITELEARN_DB_PASSWORD` | 密码    |
  | `LITELEARN_DB_NAME`     | 数据库名  |

## 使用说明

- 顶部选择技术栈, 输入关键词后回车或点击「搜索」
- 输入纯数字可精确匹配片段编号; 输入 `000` 或留空查看全部
- 点击左侧结果行, 在右侧查看/编辑代码与说明, 可复制、保存、删除
- 「新增数据」向当前技术栈插入新片段; 「新建技术栈」创建新的分类
- 「导出备份」可将全部数据导出为 JSON / CSV 文件, 保存位置可自定义
- 「导入备份」可将导出的备份文件重新导入, 已存在的数据自动跳过, 不覆盖原有内容
- 右上角可切换深色/浅色模式, 偏好会自动保存
- 「SQL 控制台」支持执行单条 SQL 语句, 请谨慎操作
