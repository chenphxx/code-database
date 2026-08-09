import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

// ============ 全局状态 ============
let currentStackId = 0; // 当前选中的技术栈编号
let currentSnippetId = null; // 当前选中的片段编号
let snippetRows = []; // 搜索结果缓存
let stacks = []; // 技术栈列表缓存

// ============ 页面元素 ============
const ui = {
  stackSelect: document.getElementById("stack-select"),
  searchInput: document.getElementById("search-input"),
  resultBody: document.getElementById("result-body"),
  resultEmpty: document.getElementById("result-empty"),
  codeId: document.getElementById("code-id"),
  codeEdit: document.getElementById("code-edit"),
  commentEdit: document.getElementById("comment-edit"),
  statusbar: document.getElementById("statusbar"),

  newDataDialog: document.getElementById("new-data-dialog"),
  newDataStack: document.getElementById("new-data-stack"),
  newDataZh: document.getElementById("new-data-zh-index"),
  newDataEn: document.getElementById("new-data-en-index"),
  newDataCode: document.getElementById("new-data-code"),
  newDataComment: document.getElementById("new-data-comment"),

  newStackDialog: document.getElementById("new-stack-dialog"),
  newStackName: document.getElementById("new-stack-name"),
  newStackDescription: document.getElementById("new-stack-description"),

  settingsDialog: document.getElementById("settings-dialog"),
  settingHost: document.getElementById("setting-host"),
  settingPort: document.getElementById("setting-port"),
  settingUser: document.getElementById("setting-user"),
  settingPassword: document.getElementById("setting-password"),
  settingDatabase: document.getElementById("setting-database"),
  settingStatus: document.getElementById("setting-status"),

  exportDialog: document.getElementById("export-dialog"),
  exportFormat: document.getElementById("export-format"),
  exportPath: document.getElementById("export-path"),
  exportStatus: document.getElementById("export-status"),

  importDialog: document.getElementById("import-dialog"),
  importPath: document.getElementById("import-path"),
  importStatus: document.getElementById("import-status"),

  sqlDialog: document.getElementById("sql-dialog"),
  sqlInput: document.getElementById("sql-input"),
  sqlResult: document.getElementById("sql-result"),

  confirmDialog: document.getElementById("confirm-dialog"),
  confirmTitle: document.getElementById("confirm-title"),
  confirmMessage: document.getElementById("confirm-message"),
};

// ============ 通用工具 ============

/**
 * @brief 显示状态栏消息
 *
 * @param message 消息内容
 * @param timeout 显示时长(毫秒), 默认 3000
 * @return 无
 */
function show_status(message, timeout = 3000) {
  ui.statusbar.textContent = message;
  clearTimeout(show_status.timer);
  show_status.timer = setTimeout(() => {
    ui.statusbar.textContent = "就绪";
  }, timeout);
}

/**
 * @brief HTML 转义, 防止数据库内容破坏页面结构
 *
 * @param text 原始文本
 * @return 转义后的文本
 */
function escape_html(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * @brief 复制文本到剪贴板
 *
 * @param text 要复制的文本
 * @return 是否复制成功
 */
async function copy_text(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (error) {
    // WebView2 环境下剪贴板 API 不可用时的兜底方案
    const textarea = document.createElement("textarea");
    textarea.value = text;
    document.body.appendChild(textarea);
    textarea.select();
    const ok = document.execCommand("copy");
    textarea.remove();
    return ok;
  }
}

/**
 * @brief 弹出确认框
 *
 * @param message 提示内容
 * @return Promise, 解析为用户是否确认
 */
function show_confirm(message) {
  return new Promise((resolve) => {
    // 先关闭其它已打开的弹窗, 避免多个模态弹窗叠加
    for (const dialog of document.querySelectorAll("dialog[open]")) {
      if (dialog !== ui.confirmDialog) {
        dialog.close();
      }
    }
    ui.confirmMessage.textContent = message;
    ui.confirmDialog.showModal();
    confirm_ok_handler = () => {
      cleanup();
      resolve(true);
    };
    confirm_cancel_handler = () => {
      cleanup();
      resolve(false);
    };
    function cleanup() {
      ui.confirmDialog.removeEventListener("click", confirm_cancel_handler);
      ui.confirmDialog.close();
    }
  });
}

let confirm_ok_handler = null;
let confirm_cancel_handler = null;

/**
 * @brief 打开弹窗, 先关闭其它已打开的弹窗
 *
 * 浏览器不允许同时存在多个模态弹窗, 统一在此处理
 *
 * @param dialog 要打开的弹窗元素
 * @return 无
 */
function show_dialog(dialog) {
  for (const item of document.querySelectorAll("dialog[open]")) {
    if (item !== dialog) {
      item.close();
    }
  }
  dialog.showModal();
}

// ============ 主题切换 ============

/**
 * @brief 应用主题
 *
 * @param theme 主题名称, light 或 dark
 * @return 无
 */
function apply_theme(theme) {
  document.documentElement.setAttribute("data-theme", theme);
  const button = document.getElementById("btn-theme");
  button.textContent = theme === "dark" ? "☀️ 浅色" : "🌙 深色";
  localStorage.setItem("litelearn-theme", theme);
}

/**
 * @brief 初始化主题, 读取上次保存的偏好
 *
 * @return 无
 */
function init_theme() {
  const saved = localStorage.getItem("litelearn-theme");
  apply_theme(saved === "dark" ? "dark" : "light");
}

/**
 * @brief 主题切换按钮点击事件
 *
 * @return 无
 */
function theme_toggle_clicked() {
  const current = document.documentElement.getAttribute("data-theme") || "light";
  apply_theme(current === "dark" ? "light" : "dark");
}

// ============ 弹窗拖动与缩放 ============

/**
 * @brief 使弹窗可通过指定手柄区域拖动
 *
 * @param dialog 弹窗元素
 * @param handle 拖动手柄元素
 * @return 无
 */
function enable_drag(dialog, handle) {
  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    const rect = dialog.getBoundingClientRect();
    const startX = event.clientX;
    const startY = event.clientY;
    const originLeft = rect.left;
    const originTop = rect.top;
    // 捕获指针, 使拖拽在鼠标移出窗口后仍能继续
    try {
      handle.setPointerCapture(event.pointerId);
    } catch (error) {
      // 忽略捕获失败
    }

    const on_move = (move_event) => {
      dialog.style.position = "fixed";
      dialog.style.margin = "0";
      dialog.style.left = `${originLeft + move_event.clientX - startX}px`;
      dialog.style.top = `${originTop + move_event.clientY - startY}px`;
    };
    const on_end = () => {
      handle.removeEventListener("pointermove", on_move);
      handle.removeEventListener("pointerup", on_end);
      handle.removeEventListener("pointercancel", on_end);
    };
    handle.addEventListener("pointermove", on_move);
    handle.addEventListener("pointerup", on_end);
    handle.addEventListener("pointercancel", on_end);
  });
}

/**
 * @brief 使弹窗可通过右下角手柄调整大小
 *
 * @param dialog 弹窗元素
 * @param handle 缩放手柄元素
 * @param minWidth 最小宽度
 * @param minHeight 最小高度
 * @return 无
 */
function enable_resize(dialog, handle, minWidth, minHeight) {
  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startY = event.clientY;
    const startWidth = dialog.offsetWidth;
    const startHeight = dialog.offsetHeight;
    // 捕获指针, 使缩放调整在鼠标移出窗口后仍能继续
    try {
      handle.setPointerCapture(event.pointerId);
    } catch (error) {
      // 忽略捕获失败
    }

    const on_move = (move_event) => {
      const width = Math.max(minWidth, startWidth + move_event.clientX - startX);
      const height = Math.max(minHeight, startHeight + move_event.clientY - startY);
      dialog.style.width = `${width}px`;
      dialog.style.height = `${height}px`;
    };
    const on_end = () => {
      handle.removeEventListener("pointermove", on_move);
      handle.removeEventListener("pointerup", on_end);
      handle.removeEventListener("pointercancel", on_end);
    };
    handle.addEventListener("pointermove", on_move);
    handle.addEventListener("pointerup", on_end);
    handle.addEventListener("pointercancel", on_end);
  });
}

// ============ 导出备份 ============

/**
 * @brief 打开导出备份弹窗
 *
 * @return 无
 */
function export_clicked() {
  ui.exportPath.value = "";
  ui.exportStatus.textContent = "";
  ui.exportStatus.classList.remove("error");
  show_dialog(ui.exportDialog);
}

/**
 * @brief 浏览选择导出保存位置
 *
 * @return 无
 */
async function export_browse_clicked() {
  const format = ui.exportFormat.value;
  const extension = format === "csv" ? "csv" : "json";
  try {
    const path = await save({
      defaultPath: format === "csv" ? "litelearn-backup.csv" : "litelearn-backup.json",
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
    });
    if (path) {
      ui.exportPath.value = path;
      ui.exportStatus.textContent = "";
      ui.exportStatus.classList.remove("error");
    }
  } catch (error) {
    ui.exportStatus.textContent = "选择保存位置失败: " + error;
    ui.exportStatus.classList.add("error");
  }
}

/**
 * @brief 导出备份确认按钮事件
 *
 * @return 无
 */
async function export_save_clicked() {
  const format = ui.exportFormat.value;
  const path = ui.exportPath.value.trim();
  if (!path) {
    ui.exportStatus.textContent = "请先选择保存位置";
    ui.exportStatus.classList.add("error");
    return;
  }
  ui.exportStatus.textContent = "正在导出...";
  ui.exportStatus.classList.remove("error");
  try {
    const result = await invoke("export_backup", { format: format, path: path });
    ui.exportStatus.textContent = `导出成功: ${result.stacks} 个技术栈, ${result.snippets} 条片段`;
    show_status("备份导出成功", 3000);
  } catch (error) {
    ui.exportStatus.textContent = "导出失败: " + error;
    ui.exportStatus.classList.add("error");
  }
}

// ============ 导入备份 ============

/**
 * @brief 打开导入备份弹窗
 *
 * @return 无
 */
function import_clicked() {
  ui.importPath.value = "";
  ui.importStatus.textContent = "";
  ui.importStatus.classList.remove("error");
  show_dialog(ui.importDialog);
}

/**
 * @brief 浏览选择备份文件, 并根据扩展名识别格式
 *
 * @return 无
 */
async function import_browse_clicked() {
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "备份文件", extensions: ["json", "csv"] }],
    });
    if (path) {
      ui.importPath.value = path;
      const format = path.toLowerCase().endsWith(".csv") ? "csv" : "json";
      ui.importStatus.textContent = `已选择 ${format.toUpperCase()} 格式备份文件`;
      ui.importStatus.classList.remove("error");
    }
  } catch (error) {
    ui.importStatus.textContent = "选择文件失败: " + error;
    ui.importStatus.classList.add("error");
  }
}

/**
 * @brief 导入备份确认按钮事件
 *
 * @return 无
 */
async function import_save_clicked() {
  const path = ui.importPath.value.trim();
  if (!path) {
    ui.importStatus.textContent = "请先选择备份文件";
    ui.importStatus.classList.add("error");
    return;
  }
  const format = path.toLowerCase().endsWith(".csv") ? "csv" : "json";
  const ok = await show_confirm(
    "导入将合并现有数据: 已存在的技术栈与片段会跳过, 不会覆盖已有数据。是否继续?"
  );
  if (!ok) {
    return;
  }
  ui.importStatus.textContent = "正在导入...";
  ui.importStatus.classList.remove("error");
  try {
    const result = await invoke("import_backup", { format: format, path: path });
    ui.importStatus.textContent =
      `导入完成: 新建技术栈 ${result.stacks_created} 个, ` +
      `已存在 ${result.stacks_existed} 个, 导入片段 ${result.snippets_imported} 条, ` +
      `跳过 ${result.snippets_skipped} 条`;
    show_status("备份导入完成", 3000);
    await load_stacks();
    await search_button_clicked();
  } catch (error) {
    ui.importStatus.textContent = "导入失败: " + error;
    ui.importStatus.classList.add("error");
  }
}

// ============ 技术栈 ============

/**
 * @brief 加载技术栈列表到下拉框
 *
 * @return 无
 */
async function load_stacks() {
  try {
    stacks = await invoke("list_stacks");
  } catch (error) {
    show_status("技术栈列表加载失败: " + error, 5000);
    return;
  }

  const previous = ui.stackSelect.value;
  ui.stackSelect.innerHTML = "";
  for (const stack of stacks) {
    const option = document.createElement("option");
    option.value = stack.id;
    option.textContent = `${stack.name} (${stack.count})`;
    ui.stackSelect.appendChild(option);
  }

  if (stacks.length > 0) {
    // 优先恢复之前的选中项
    if (previous && [...ui.stackSelect.options].some((opt) => opt.value === previous)) {
      ui.stackSelect.value = previous;
    }
    currentStackId = Number(ui.stackSelect.value);
  } else {
    currentStackId = 0;
    ui.resultBody.innerHTML = "";
    ui.resultEmpty.style.display = "block";
  }
}

/**
 * @brief 技术栈下拉框切换事件
 *
 * @return 无
 */
async function stack_switch_activated() {
  currentStackId = Number(ui.stackSelect.value) || 0;
  if (currentStackId === 0) {
    show_status("请先创建技术栈", 3000);
    return;
  }
  await search_button_clicked();
}

// ============ 搜索 ============

/**
 * @brief 搜索按钮点击事件
 *
 * @return 无
 */
async function search_button_clicked() {
  const keyword = ui.searchInput.value.trim();
  currentStackId = Number(ui.stackSelect.value) || 0;

  if (currentStackId === 0) {
    show_status("请先选择技术栈", 3000);
    return;
  }

  try {
    snippetRows = await invoke("search_snippets", {
      stackId: currentStackId,
      keyword: keyword,
    });
  } catch (error) {
    show_status("查询失败: " + error, 5000);
    return;
  }

  render_result_table();
  show_status(`查询完成, 共 ${snippetRows.length} 条结果`, 2000);
}

/**
 * @brief 渲染搜索结果表格
 *
 * @return 无
 */
function render_result_table() {
  ui.resultBody.innerHTML = "";
  if (snippetRows.length === 0) {
    ui.resultEmpty.style.display = "block";
    return;
  }
  ui.resultEmpty.style.display = "none";

  for (const row of snippetRows) {
    const tr = document.createElement("tr");
    tr.dataset.id = row.id;
    tr.innerHTML =
      `<td>${row.id}</td>` +
      `<td>${escape_html(row.zh_index)}</td>` +
      `<td>${escape_html(row.en_index)}</td>` +
      `<td>${escape_html(row.updated_at)}</td>`;
    tr.addEventListener("click", () => row_clicked(Number(row.id)));
    ui.resultBody.appendChild(tr);
  }
}

/**
 * @brief 点击表格行, 展示片段详情
 *
 * @param id 片段编号
 * @return 无
 */
function row_clicked(id) {
  const row = snippetRows.find((item) => item.id === id);
  if (!row) {
    return;
  }
  currentSnippetId = id;
  ui.codeEdit.value = row.code_snippet;
  ui.commentEdit.value = row.zh_comment;
  ui.codeId.textContent = "#" + id;

  // 高亮当前选中行
  for (const tr of ui.resultBody.children) {
    tr.classList.toggle("selected", Number(tr.dataset.id) === id);
  }
}

// ============ 详情面板操作 ============

/**
 * @brief 复制代码片段
 *
 * @return 无
 */
async function code_copy_clicked() {
  const text = ui.codeEdit.value;
  if (!text) {
    show_status("没有可复制的内容", 2000);
    return;
  }
  const ok = await copy_text(text);
  show_status(ok ? "代码已复制到剪贴板" : "复制失败", 2000);
}

/**
 * @brief 复制中文说明
 *
 * @return 无
 */
async function comment_copy_clicked() {
  const text = ui.commentEdit.value;
  if (!text) {
    show_status("没有可复制的内容", 2000);
    return;
  }
  const ok = await copy_text(text);
  show_status(ok ? "说明已复制到剪贴板" : "复制失败", 2000);
}

/**
 * @brief 获取当前选中片段的缓存行
 *
 * @return 缓存行对象, 不存在时返回 null
 */
function current_row() {
  return snippetRows.find((item) => item.id === currentSnippetId) || null;
}

/**
 * @brief 保存代码片段内容到数据库
 *
 * @return 无
 */
async function code_save_clicked() {
  if (currentSnippetId === null) {
    show_status("请先选择一条记录", 3000);
    return;
  }
  const row = current_row();
  try {
    await invoke("update_snippet", {
      id: currentSnippetId,
      zhIndex: row.zh_index,
      enIndex: row.en_index,
      codeSnippet: ui.codeEdit.value,
      zhComment: row.zh_comment,
    });
    show_status("代码保存成功", 2000);
    await search_button_clicked();
  } catch (error) {
    show_status("保存失败: " + error, 5000);
  }
}

/**
 * @brief 保存中文说明内容到数据库
 *
 * @return 无
 */
async function comment_save_clicked() {
  if (currentSnippetId === null) {
    show_status("请先选择一条记录", 3000);
    return;
  }
  const row = current_row();
  try {
    await invoke("update_snippet", {
      id: currentSnippetId,
      zhIndex: row.zh_index,
      enIndex: row.en_index,
      codeSnippet: row.code_snippet,
      zhComment: ui.commentEdit.value,
    });
    show_status("说明保存成功", 2000);
    await search_button_clicked();
  } catch (error) {
    show_status("保存失败: " + error, 5000);
  }
}

/**
 * @brief 清空代码片段输入框
 *
 * @return 无
 */
function code_clear_clicked() {
  ui.codeEdit.value = "";
  show_status("已清空代码输入框", 1500);
}

/**
 * @brief 清空中文说明输入框
 *
 * @return 无
 */
function comment_clear_clicked() {
  ui.commentEdit.value = "";
  show_status("已清空说明输入框", 1500);
}

/**
 * @brief 删除当前记录
 *
 * @return 无
 */
async function delete_clicked() {
  if (currentSnippetId === null) {
    show_status("请先选择一条记录", 3000);
    return;
  }
  const ok = await show_confirm(`确定删除记录 #${currentSnippetId} 吗? 该操作不可恢复`);
  if (!ok) {
    return;
  }
  try {
    await invoke("delete_snippet", { id: currentSnippetId });
    currentSnippetId = null;
    ui.codeEdit.value = "";
    ui.commentEdit.value = "";
    ui.codeId.textContent = "";
    show_status("删除成功", 2000);
    await search_button_clicked();
  } catch (error) {
    show_status("删除失败: " + error, 5000);
  }
}

// ============ 新增数据 ============

/**
 * @brief 打开新增数据弹窗
 *
 * @return 无
 */
function new_data_clicked() {
  show_dialog(ui.newDataDialog);
  ui.newDataStack.innerHTML = "";
  for (const stack of stacks) {
    const option = document.createElement("option");
    option.value = stack.id;
    option.textContent = stack.name;
    ui.newDataStack.appendChild(option);
  }
  ui.newDataStack.value = String(currentStackId);
  ui.newDataZh.value = "";
  ui.newDataEn.value = "";
  ui.newDataCode.value = "";
  ui.newDataComment.value = "";
  ui.newDataZh.focus();
}

/**
 * @brief 新增数据保存按钮事件
 *
 * @return 无
 */
async function new_data_save_clicked() {
  const zhIndex = ui.newDataZh.value.trim();
  const enIndex = ui.newDataEn.value.trim();
  const code = ui.newDataCode.value;
  const comment = ui.newDataComment.value;
  const stackId = Number(ui.newDataStack.value) || 0;

  if (stackId === 0) {
    show_status("请先创建技术栈", 3000);
    return;
  }
  if (!zhIndex && !enIndex) {
    show_status("中文索引与英文索引至少填写一项", 3000);
    return;
  }
  if (!code.trim()) {
    show_status("代码片段不能为空", 3000);
    return;
  }

  try {
    await invoke("add_snippet", {
      stackId: stackId,
      zhIndex: zhIndex,
      enIndex: enIndex,
      codeSnippet: code,
      zhComment: comment,
    });
    ui.newDataDialog.close();
    show_status("保存成功", 2000);
    // 切换到新增数据所属的技术栈并刷新列表
    ui.stackSelect.value = String(stackId);
    await search_button_clicked();
  } catch (error) {
    show_status("保存失败: " + error, 5000);
  }
}

// ============ 新建技术栈 ============

/**
 * @brief 打开新建技术栈弹窗
 *
 * @return 无
 */
function new_stack_clicked() {
  ui.newStackName.value = "";
  ui.newStackDescription.value = "";
  show_dialog(ui.newStackDialog);
  ui.newStackName.focus();
}

/**
 * @brief 新建技术栈确认按钮事件
 *
 * @return 无
 */
async function new_stack_save_clicked() {
  const name = ui.newStackName.value.trim();
  const description = ui.newStackDescription.value.trim();
  if (!name) {
    show_status("技术栈名称不能为空", 3000);
    return;
  }
  try {
    await invoke("create_stack", { name: name, description: description });
    ui.newStackDialog.close();
    show_status(`技术栈 ${name} 创建成功`, 2000);
    await load_stacks();
    await search_button_clicked();
  } catch (error) {
    show_status("创建失败: " + error, 5000);
  }
}

// ============ 数据库设置 ============

/**
 * @brief 打开设置弹窗并填充当前配置
 *
 * @return 无
 */
async function settings_clicked() {
  ui.settingStatus.textContent = "";
  ui.settingStatus.classList.remove("error");
  try {
    const config = await invoke("get_config");
    ui.settingHost.value = config.host;
    ui.settingPort.value = config.port;
    ui.settingUser.value = config.user;
    ui.settingPassword.value = config.password;
    ui.settingDatabase.value = config.database;
  } catch (error) {
    ui.settingStatus.textContent = "配置读取失败: " + error;
    ui.settingStatus.classList.add("error");
  }
  show_dialog(ui.settingsDialog);
}

/**
 * @brief 从设置表单读取配置
 *
 * @return 配置对象
 */
function read_settings_form() {
  return {
    host: ui.settingHost.value.trim(),
    port: Number(ui.settingPort.value) || 3306,
    user: ui.settingUser.value.trim(),
    password: ui.settingPassword.value,
    database: ui.settingDatabase.value.trim(),
  };
}

/**
 * @brief 测试连接按钮事件
 *
 * @return 无
 */
async function setting_test_clicked() {
  ui.settingStatus.textContent = "正在测试连接...";
  ui.settingStatus.classList.remove("error");
  try {
    const version = await invoke("test_connection", { config: read_settings_form() });
    ui.settingStatus.textContent = "连接成功, MySQL 版本: " + version;
  } catch (error) {
    ui.settingStatus.textContent = "连接失败: " + error;
    ui.settingStatus.classList.add("error");
  }
}

/**
 * @brief 保存设置按钮事件
 *
 * @return 无
 */
async function setting_save_clicked() {
  ui.settingStatus.textContent = "正在保存并重连...";
  ui.settingStatus.classList.remove("error");
  try {
    await invoke("save_config", { config: read_settings_form() });
    ui.settingStatus.textContent = "设置已保存并重新连接";
    show_status("数据库设置已更新", 2000);
    await load_stacks();
    await search_button_clicked();
  } catch (error) {
    ui.settingStatus.textContent = "保存失败: " + error;
    ui.settingStatus.classList.add("error");
  }
}

// ============ SQL 控制台 ============

/**
 * @brief 打开 SQL 控制台弹窗
 *
 * @return 无
 */
function sql_clicked() {
  ui.sqlResult.textContent = "";
  show_dialog(ui.sqlDialog);
  ui.sqlInput.focus();
}

/**
 * @brief SQL 控制台执行按钮事件
 *
 * @return 无
 */
async function sql_run_clicked() {
  const sql = ui.sqlInput.value.trim();
  if (!sql) {
    ui.sqlResult.textContent = "请输入要执行的 SQL 语句";
    return;
  }
  ui.sqlResult.textContent = "正在执行...";
  try {
    const result = await invoke("execute_sql", { sql: sql });
    let text = "";
    if (result.columns.length > 0) {
      text += result.columns.join("\t") + "\n";
      for (const row of result.rows) {
        text += row.join("\t") + "\n";
      }
      text += `\n共 ${result.rows.length} 行`;
    } else {
      text += `执行成功, 影响 ${result.affected} 行`;
    }
    ui.sqlResult.textContent = text;
  } catch (error) {
    ui.sqlResult.textContent = "执行失败: " + error;
  }
}

/**
 * @brief SQL 控制台清空按钮事件
 *
 * @return 无
 */
function sql_clear_clicked() {
  ui.sqlInput.value = "";
  ui.sqlResult.textContent = "";
}

// ============ 初始化与事件绑定 ============

/**
 * @brief 绑定页面事件
 *
 * @return 无
 */
function bind_events() {
  document.getElementById("btn-search").addEventListener("click", search_button_clicked);
  ui.stackSelect.addEventListener("change", stack_switch_activated);
  document.getElementById("btn-theme").addEventListener("click", theme_toggle_clicked);
  ui.searchInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      search_button_clicked();
    }
  });

  document.getElementById("btn-code-copy").addEventListener("click", code_copy_clicked);
  document.getElementById("btn-code-save").addEventListener("click", code_save_clicked);
  document.getElementById("btn-code-clear").addEventListener("click", code_clear_clicked);
  document.getElementById("btn-comment-copy").addEventListener("click", comment_copy_clicked);
  document.getElementById("btn-comment-save").addEventListener("click", comment_save_clicked);
  document.getElementById("btn-comment-clear").addEventListener("click", comment_clear_clicked);
  document.getElementById("btn-delete").addEventListener("click", delete_clicked);

  document.getElementById("btn-new-data").addEventListener("click", new_data_clicked);
  document.getElementById("btn-new-data-save").addEventListener("click", new_data_save_clicked);
  document.getElementById("btn-new-data-cancel").addEventListener("click", () => ui.newDataDialog.close());

  document.getElementById("btn-new-stack").addEventListener("click", new_stack_clicked);
  document.getElementById("btn-new-stack-save").addEventListener("click", new_stack_save_clicked);
  document.getElementById("btn-new-stack-cancel").addEventListener("click", () => ui.newStackDialog.close());

  document.getElementById("btn-export").addEventListener("click", export_clicked);
  document.getElementById("btn-export-browse").addEventListener("click", export_browse_clicked);
  document.getElementById("btn-export-save").addEventListener("click", export_save_clicked);
  document.getElementById("btn-export-cancel").addEventListener("click", () => ui.exportDialog.close());
  ui.exportFormat.addEventListener("change", () => {
    // 切换格式后清空已选路径, 避免扩展名不一致
    ui.exportPath.value = "";
    ui.exportStatus.textContent = "";
    ui.exportStatus.classList.remove("error");
  });

  document.getElementById("btn-import").addEventListener("click", import_clicked);
  document.getElementById("btn-import-browse").addEventListener("click", import_browse_clicked);
  document.getElementById("btn-import-save").addEventListener("click", import_save_clicked);
  document.getElementById("btn-import-cancel").addEventListener("click", () => ui.importDialog.close());

  document.getElementById("btn-settings").addEventListener("click", settings_clicked);
  document.getElementById("btn-setting-test").addEventListener("click", setting_test_clicked);
  document.getElementById("btn-setting-save").addEventListener("click", setting_save_clicked);
  document.getElementById("btn-setting-cancel").addEventListener("click", () => ui.settingsDialog.close());
  document.getElementById("btn-toggle-password").addEventListener("click", toggle_password_clicked);

  document.getElementById("btn-sql").addEventListener("click", sql_clicked);
  document.getElementById("btn-sql-run").addEventListener("click", sql_run_clicked);
  document.getElementById("btn-sql-clear").addEventListener("click", sql_clear_clicked);
  document.getElementById("btn-sql-close").addEventListener("click", () => ui.sqlDialog.close());

  document.getElementById("btn-confirm-ok").addEventListener("click", () => {
    if (confirm_ok_handler) {
      confirm_ok_handler();
    }
  });
  document.getElementById("btn-confirm-cancel").addEventListener("click", () => {
    if (confirm_cancel_handler) {
      confirm_cancel_handler();
    }
  });

  // 所有弹窗支持拖动与缩放
  const resizable_dialogs = [
    { dialog: ui.newDataDialog, minWidth: 480, minHeight: 420 },
    { dialog: ui.newStackDialog, minWidth: 380, minHeight: 240 },
    { dialog: ui.settingsDialog, minWidth: 420, minHeight: 420 },
    { dialog: ui.exportDialog, minWidth: 400, minHeight: 260 },
    { dialog: ui.importDialog, minWidth: 400, minHeight: 240 },
    { dialog: ui.sqlDialog, minWidth: 520, minHeight: 400 },
    { dialog: ui.confirmDialog, minWidth: 340, minHeight: 200 },
  ];
  for (const config of resizable_dialogs) {
    const drag_handle = config.dialog.querySelector(".drag-handle");
    const resize_handle = config.dialog.querySelector(".resize-handle");
    if (drag_handle) {
      enable_drag(config.dialog, drag_handle);
    }
    if (resize_handle) {
      enable_resize(config.dialog, resize_handle, config.minWidth, config.minHeight);
    }
  }
}

/**
 * @brief 密码框明文/密文切换按钮事件
 *
 * @return 无
 */
function toggle_password_clicked() {
  const input = ui.settingPassword;
  const show = input.type === "password";
  input.type = show ? "text" : "password";
  document.getElementById("btn-toggle-password").textContent = show ? "🙈" : "👁";
}

/**
 * @brief 应用初始化入口
 *
 * @return 无
 */
async function init() {
  init_theme();
  bind_events();
  await load_stacks();
  if (currentStackId !== 0) {
    await search_button_clicked();
  }
}

init();
