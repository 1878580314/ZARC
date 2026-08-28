# ZARC Studio

## 中文

ZARC Studio 是一款基于 **Rust + Tauri 2 + Svelte 5** 构建的现代化跨平台压缩工具。它将高性能的 **zstd** 压缩引擎与严苛的现代加密标准结合，提供兼具极致速度、高压缩比与数据安全的桌面解压缩体验。

### 🌟 核心亮点

- **极速与全面**：采用高效的 zstd 算法与 tar 格式打包（支持 `.zst` 与 `.tar.zst`）。不仅支持大文件自动分卷和一键生成 Windows 自解压程序（SFX .exe），还内置了压缩性能基准测试（Benchmark），方便快速挑选最适合当前硬件的压缩等级。
- **坚如磐石的安全**：拒绝弱加密。密码绝不直接落地，而是通过高强度的 **Argon2id** 算法派生密钥，并由 **XChaCha20-Poly1305** 提供认证加密，同时辅以 **BLAKE3** 高速哈希校验。解压时采用“临时目录事务机制”，如果解压中途出错或发生路径冲突，不会误损坏原有的文件。
- **流畅的桌面交互**：提供直观的归档文件内容预览，任务进行中支持实时查看速度与吞吐量，并可随时无损取消任务。

### 🚀 v0.1.1 更新要点

本版本重点提升了底层兼容性与容错能力：
全面适配了 Windows 下的特殊路径与设备保留名（例如目录中的 `NUL`）；加密归档格式迎来升级，在保持向下兼容的同时加入了 Argon2id 动态参数记录与防恶意篡改校验；进一步强化了分卷归档、自解压生成、符号链接保留与“安全防覆盖”机制，并重构了桌面端的任务状态管理与基准测试交互。

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

## English

ZARC Studio is a modern, cross-platform compression tool powered by **Rust, Tauri 2, and Svelte 5**. It pairs the lightning-fast **zstd** engine with state-of-the-art cryptography to deliver blazing speed, high compression ratios, and rock-solid data protection in an intuitive desktop interface.

### 🌟 Key Highlights

- **Fast & Versatile Packaging**: Compress files and folders into `.zst` / `.tar.zst` using modern zstd efficiency. It supports multi-volume archives for large files, one-click Windows self-extracting (SFX) executables, and a built-in benchmark tool to help you easily find the sweet spot between speed and compression ratio.
- **Battle-Tested Security**: Your master password is never stored or used directly. Keys are derived via memory-hard **Argon2id** and encrypted with **XChaCha20-Poly1305** authenticated encryption, alongside **BLAKE3** integrity hashing. The extraction engine uses safe transactional staging—if an operation fails or runs into a path conflict, your existing files will never be corrupted.
- **Smooth Desktop Experience**: Features an instant archive content preview, real-time throughput and progress monitoring, and seamless task cancellation without leaving orphan files.

### 🚀 What's New in v0.1.1

This update focuses on deep reliability and filesystem compatibility:
Added robust Windows support for verbatim paths and reserved device names (such as `NUL` in source trees); upgraded the encrypted archive structure with tamper-resistant Argon2id parameter validation while retaining backward compatibility; and hardened multi-volume handling, SFX extraction, symlink preservation, and task state management across the UI.

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
