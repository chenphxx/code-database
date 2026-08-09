#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/** 发布版本下隐藏 Windows 控制台窗口 */
/** @brief 程序入口 */
fn main() {
    litelearn_lib::run()
}
