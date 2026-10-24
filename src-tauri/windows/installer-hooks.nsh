; NSIS 安装器钩子（tauri.conf.json bundle.windows.nsis.installerHooks）。
;
; 数据目录 tmcl-data\ 在安装目录下（docs/contracts/data-layout.md），默认卸载只删
; 安装时写入的文件，存档与实例会保留。用户在卸载界面勾选“删除应用数据”时一并删除；
; 自动更新触发的卸载（/UPDATE）永远不删。

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$INSTDIR\tmcl-data"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
