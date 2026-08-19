# ZARC Studio

## 中文

ZARC Studio 是一个基于 **Rust + Tauri + Svelte** 的跨平台高性能压缩工具。

### 最新更新

- 加强 Windows 文件系统兼容性，支持目录中的 `NUL` 等保留设备名与 verbatim 路径。
- 升级加密归档格式，记录 Argon2id 参数并增加恶意参数/分块长度校验，同时兼容旧版加密归档。
- 强化分卷、SFX、事务式解压、输出覆盖保护、符号链接与任务取消等可靠性处理。
- 重构桌面界面与任务状态管理，完善归档预览、Benchmark、快捷操作和结果展示。

### 功能

- zstd 高性能压缩与解压
- 文件与目录归档（`.zst` / `.tar.zst`）
- XChaCha20-Poly1305 加密
- Argon2id 密码派生
- 大文件分卷归档
- Windows 自解压 EXE
- 压缩等级 Benchmark
- BLAKE3 哈希校验信息
- 实时速度、进度和任务取消

### 技术栈

- Backend: Rust
- Desktop Framework: Tauri 2
- Frontend: Svelte 5 + TypeScript
- Compression: zstd
- Archive: tar

### 构建

需要安装：

- Node.js 20+
- Rust stable
- Tauri 2 依赖环境

```bash
cd zarc-desktop
npm install
npm run tauri build
```

开发运行：

```bash
npm run tauri dev
```

### 安全设计

- 密码不会直接作为加密密钥使用
- 使用 Argon2id 派生密钥
- 使用 XChaCha20-Poly1305 提供认证加密
- 解压采用临时目录提交机制，避免失败覆盖用户数据
- 输出路径冲突会被拒绝

### 项目状态

当前版本重点关注稳定压缩、加密归档和桌面体验。

---

# ZARC Studio

## English

ZARC Studio is a cross-platform high-performance compression application built with **Rust + Tauri + Svelte**.

### Latest Update

- Improved Windows filesystem compatibility, including verbatim paths and reserved names such as `NUL` inside source trees.
- Upgraded the encrypted archive format to store Argon2id parameters and validate hostile KDF/chunk values while retaining legacy compatibility.
- Hardened multi-volume archives, SFX handling, transactional extraction, overwrite protection, symlink preservation, and task cancellation.
- Refined the desktop UI and task state architecture with better archive preview, benchmark workflow, shortcuts, and result presentation.

### Features

- High-performance zstd compression and extraction
- File and directory archives (`.zst` / `.tar.zst`)
- XChaCha20-Poly1305 authenticated encryption
- Argon2id password-based key derivation
- Multi-volume archive support
- Windows self-extracting executable archives
- Compression benchmark
- BLAKE3 hash reporting
- Real-time progress, throughput and cancellation

### Tech Stack

- Backend: Rust
- Desktop Framework: Tauri 2
- Frontend: Svelte 5 + TypeScript
- Compression: zstd
- Archive Format: tar

### Build

Requirements:

- Node.js 20+
- Rust stable
- Tauri 2 system dependencies

```bash
cd zarc-desktop
npm install
npm run tauri build
```

Run in development mode:

```bash
npm run tauri dev
```

### Security Design

- Passwords are never used directly as encryption keys
- Keys are derived with Argon2id
- Encryption uses XChaCha20-Poly1305 authenticated encryption
- Extraction uses transactional temporary output to protect existing data
- Unsafe path conflicts are rejected

### Project Status

The current version focuses on reliable compression, encrypted archives and desktop usability.
