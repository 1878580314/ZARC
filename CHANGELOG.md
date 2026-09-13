# Changelog

## v0.1.3 — 2026-09-13

本次更新集中修复归档完整性、取消任务、Windows 路径与 SFX 检测，并改善大文件处理和桌面交互。

### 修复与优化

- 目录遍历失败时中止压缩，避免遗漏文件却报告成功；加强输出路径保护。
- 修复 tar 解压取消时的重复重试，提交解压结果前完整验证外层流；新归档启用 zstd 校验。
- 修复分卷及大写扩展名的默认解压路径，统一输出扩展名和已有目标文件夹语义。
- 修复 Windows 字面 `NUL` 文件名的读取与往返。
- 新生成的大容量 SFX 在 EXE 中保存侧车标识，数据文件缺失或损坏时提供明确反馈。
- 普通归档预览和解压同步计算 BLAKE3；预览建立目录索引并分页，路径统计支持限时和取消过期请求。
- 切页保留设置与结果，成功操作后清空密码；新增最终输出路径预览、打开所在目录、简化视觉效果选项，改善表单可访问性。
- 精简字体资源，Windows EXE 体积较 v0.1.2 减少约 75%。

### 下载与验证

- `ZARC.exe`：Windows x64 单 EXE，由 Windows 原生 MSVC/Tauri Release 工具链构建。
- `SHA256SUMS.txt`：下载文件的 SHA-256 校验值。
- 本轮验证通过 Linux 46 项、Windows 43 项核心测试，以及前端交互、真实窗口压缩/解压、分卷和 SFX 往返检查。
- 历史未写入侧车标识的 SFX 宿主，单独丢失 `.payload` 后仍无法自行识别缺失状态；新生成的 SFX 已修复。

### English

Improves archive integrity, cancellation, Windows path handling, SFX detection, large-archive browsing, and desktop usability. Fixes silent directory traversal omissions, retrying cancellation errors, incomplete outer-stream validation, split/uppercase output names, and literal Windows `NUL` filenames. Adds output-path previews, reveal-in-folder, retained forms, clearer task stages, and simpler visual effects. Reduces the Windows executable size by approximately 75%.

This release provides a Windows x64 executable and SHA-256 checksums. Existing archive formats remain readable. New sidecar SFX hosts retain a manifest so missing payload files can be detected; legacy unmarked hosts retain their historical limitation.
