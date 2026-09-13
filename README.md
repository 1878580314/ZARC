# ZARC Studio

ZARC Studio 是一款基于 **Rust + Tauri 2 + Svelte 5** 构建的现代化跨平台压缩工具。它将高性能的 **zstd** 压缩引擎与严苛的现代加密标准结合，提供兼具极致速度、高压缩比与数据安全的桌面解压缩体验。

### 🌟 核心亮点

- **极速与全面**：采用高效的 zstd 算法与 tar 格式打包（支持 `.zst` 与 `.tar.zst`）。不仅支持大文件自动分卷和一键生成 Windows 自解压程序（SFX .exe），还内置了压缩性能基准测试（Benchmark），方便快速挑选最适合当前硬件的压缩等级。
- **坚如磐石的安全**：拒绝弱加密。密码绝不直接落地，而是通过高强度的 **Argon2id** 算法派生密钥，并由 **XChaCha20-Poly1305** 提供认证加密，同时辅以 **BLAKE3** 高速哈希校验。解压时采用“临时目录事务机制”，如果解压中途出错或发生路径冲突，不会误损坏原有的文件。
- **流畅的桌面交互**：提供直观的归档文件内容预览，任务进行中支持实时查看速度与吞吐量，支持取消任务，并在失败时清理未提交的解压结果。

### 🛠️ 技术栈与构建

- **技术栈**：Rust (后端) · Tauri 2 (桌面框架) · Svelte 5 + TypeScript (前端) · zstd & tar (压缩核心)

**环境要求**：Node.js 20+、Rust stable 及 Tauri 2 系统依赖。

```bash
cd zarc-desktop
npm install

# 本地开发调试
npm run tauri dev

# 打包发布版本
npm run tauri build
```

---

# ZARC Studio

ZARC Studio is a modern, cross-platform compression tool powered by **Rust, Tauri 2, and Svelte 5**. It pairs the lightning-fast **zstd** engine with state-of-the-art cryptography to deliver blazing speed, high compression ratios, and rock-solid data protection in an intuitive desktop interface.

### 🌟 Key Highlights

- **Fast & Versatile Packaging**: Compress files and folders into `.zst` / `.tar.zst` using modern zstd efficiency. It supports multi-volume archives for large files, one-click Windows self-extracting (SFX) executables, and a built-in benchmark tool to help you easily find the sweet spot between speed and compression ratio.
- **Battle-Tested Security**: Your master password is never stored or used directly. Keys are derived via memory-hard **Argon2id** and encrypted with **XChaCha20-Poly1305** authenticated encryption, alongside **BLAKE3** integrity hashing. The extraction engine uses safe transactional staging—if an operation fails or runs into a path conflict, your existing files will never be corrupted.
- **Smooth Desktop Experience**: Features a bounded archive content preview, real-time throughput and progress monitoring, and task cancellation with cleanup of uncommitted extraction results.

### 🛠️ Tech Stack & Build

- **Stack**: Rust (Backend) · Tauri 2 (Desktop Framework) · Svelte 5 + TypeScript (Frontend) · zstd & tar (Core Engine)

**Requirements**: Node.js 20+, Rust stable, and Tauri 2 build dependencies.

```bash
cd zarc-desktop
npm install

# Run in development mode
npm run tauri dev

# Build for release
npm run tauri build
```


Windows 单 EXE 构建请在 Windows 工具链环境中运行 `npm ci` 与
`npm run tauri:build:win`，产物位于 `zarc-desktop/src-tauri/target/release/ZARC.exe`。
此入口启用 Tauri 的本地资源协议；直接 `cargo build --release` 不能代替正式打包。

本轮修复统一了归档扩展名与分卷默认命名，目录遍历失败会中止压缩，tar 提交前验证外层流。
预览最多保留 100000 个条目，界面每页最多显示 100 行；目录大小统计达到 200000 条目或
约 2 秒时显示下限估计。密码只保留在当前会话内存中，操作成功后清空，其他表单设置跨页面保留。
新生成的大容量 SFX 在 EXE 中保存侧车标识；历史版本中没有此标识的孤立 EXE 无法自行判断侧车是否缺失。

For a Windows single EXE, use the Windows toolchain and run `npm ci` followed by
`npm run tauri:build:win`. This enables the embedded-resource protocol; a bare Cargo
release build is not the distribution build. Archive previews are capped at 100,000
entries and paginated at 100 rows. New sidecar SFX hosts retain their manifest when
the payload is missing; legacy unmarked hosts cannot detect a missing sidecar.
