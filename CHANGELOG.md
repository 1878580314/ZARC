# Changelog

## v1.0.0 — 2026-10-12

首个稳定版本：完成压缩管线性能优化、自绘窗口标题栏与一次全面的代码精简。

### 性能

- BLAKE3 摘要融入写入路径：压缩完成后不再整趟回读归档，"校验输出"阶段消失。
- 分卷 `fsync` 后台化：已关闭分卷在后台落盘，主线程不再逐卷停顿。
- SFX 直写容器：压缩流直接写入最终 EXE/sidecar 布局，省掉临时归档与整趟载荷拷贝。
- 大输入（≥256 MiB）自动启用 zstd 长距离匹配（LDM），并对多线程任务尺寸钳制内存上限；写入 pledged source size，压缩率与速度均有小赚。

### 界面

- 隐藏 Windows 系统标题栏，新增前端自绘标题栏：拖拽移动、双击最大化/还原，最小化/最大化/关闭按钮与应用视觉语言一致，SFX 自解压模式同样可用。

### 精简与修复

- 移除解压输出解析中的恒等分支、编码器装配三处重复与命令包装样板；测试代码从 lib.rs 拆出为独立模块。
- 修复 20 级提示文案：仅 22 级（btultra2 + 128 MiB 窗口）会自动启用 LDM，提示改为按实际阈值提示。

### 下载与验证

- `ZARC.exe`：Windows x64 单 EXE，由 GitHub Actions（windows-latest + MSVC/Tauri Release 工具链）构建。
- `SHA256SUMS.txt`：下载文件的 SHA-256 校验值。
- 验证通过 46 项核心测试、clippy 零警告与前端类型/构建检查。

### English

First stable release. Compression-pipeline performance: the BLAKE3 digest is now fused into the write path (no more full-archive re-read), per-volume `fsync` runs in the background, and SFX payloads stream directly into the final container. Large inputs (≥256 MiB) automatically enable zstd long-distance matching with a capped multithread job size, and the pledged source size is written into every frame. A custom frameless titlebar replaces the native Windows caption — drag to move, double-click to toggle maximize, with theme-consistent minimize/maximize/close controls (also in SFX mode). Code cleanup removes dead branches, triplicated encoder setup and command boilerplate; the 46-test suite, clippy and frontend checks all pass. Ships a Windows x64 executable with SHA-256 checksums, built by GitHub Actions.

## v0.1.4 — 2026-09-27

本次更新加入「关于」面板与外部链接能力，更新应用标识，并做一次轻量的体验与代码清理。

### 修复与优化

- 侧栏新增「关于」入口：显示当前版本，提供一键前往 GitHub 发布页检查更新、作者主页与源代码仓库链接，以及使用协议说明。
- 新增 `open_url` 后端命令，仅允许打开 https 链接，由系统默认浏览器承载。
- 默认压缩等级由 8 调整为 3，应用标识更新为 `com.zarc.studio`。
- 界面标语更新为「zstd 解压缩工具」，清理注释与元数据中的过期信息。

### 下载与验证

- `ZARC.exe`：Windows x64 单 EXE，由 Windows 原生 MSVC/Tauri Release 工具链构建。
- `SHA256SUMS.txt`：下载文件的 SHA-256 校验值。
- 验证通过 Linux 46 项核心测试与前端类型/构建检查。

### English

Adds an About panel with version display, update check via the GitHub releases page, author/repository links, and terms of use — backed by an `open_url` command restricted to https. Default compression level is now 3; the app identifier is updated and stale metadata/comments were cleaned up. Ships a Windows x64 executable with SHA-256 checksums; verified by the 46-test Linux core suite plus frontend type-check and build.

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
