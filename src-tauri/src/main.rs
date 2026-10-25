// 发布版在 Windows 上不弹出控制台窗口，请勿删除。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 必须在创建任何线程之前读取时区，见 tmcl_launch::init_local_offset。
    tmcl_launch::init_local_offset();
    tmcl_app_lib::run()
}
