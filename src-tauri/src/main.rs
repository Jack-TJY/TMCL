// 发布版在 Windows 上不弹出控制台窗口，请勿删除。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    disable_webkit_dmabuf();
    // 必须在创建任何线程之前读取时区，见 tmcl_launch::init_local_offset。
    tmcl_launch::init_local_offset();
    tmcl_app_lib::run()
}

/// WebKitGTK 的 DMABUF 渲染器在部分 X11 环境（vmwgfx、NVIDIA 专有驱动等）下窗口一片空白，
/// 用户没有自行设置时关闭它；Wayland 下关闭后同样正常显示。
#[cfg(target_os = "linux")]
fn disable_webkit_dmabuf() {
    const KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(KEY).is_none() {
        // SAFETY: 位于 main 开头，此时还没有创建其他线程，没有并发读写环境变量。
        unsafe { std::env::set_var(KEY, "1") };
    }
}
