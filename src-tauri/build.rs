fn main() {
    // tauri 在 Windows 上依赖 comctl32 v6 的应用清单。tauri-build 只给 bin 嵌入清单，
    // lib 的单元测试二进制会以 STATUS_ENTRYPOINT_NOT_FOUND 退出；因此关闭 tauri-build 的清单，
    // 改为把同样内容的清单嵌入所有链接目标。非 Windows 目标上 embed-resource 不做任何事。
    embed_resource::compile_for_everything("windows/app-manifest.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed the Windows application manifest");
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
