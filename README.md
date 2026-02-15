# Z-Archive Nexus (ZARC)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python: 3.8+](https://img.shields.io/badge/Python-3.8+-blue.svg)](https://www.python.org/)
[![Rust: 1.70+](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)

[English](#english) | [中文](#中文)

---

## English

ZARC (Z-Archive Nexus) is a high-performance, security-focused archiving suite. It combines the extreme speed of **Zstandard (zstd)** with robust **AES-256-GCM/XChaCha20Poly1305** encryption to provide a modern alternative for data storage and transfer.

The project offers two interfaces:
1.  **ZARC Studio (Desktop)**: A cross-platform GUI built with Tauri, Vite, and Rust.
2.  **ZARC CLI/TUI (zstd.py)**: A versatile Python script for terminal power users.

### ✨ Key Features

-   **Extreme Compression**: Powered by Zstandard, offering 22 compression levels with multi-threaded support.
-   **Security First**: 
    -   **CLI**: AES-256-GCM chunked encryption.
    -   **Desktop**: XChaCha20Poly1305 authenticated encryption with Argon2id key derivation.
-   **Intelligent Archiving**: Support for multi-volume (splitting) archives.
-   **Compression Benchmark**: Integrated tool to analyze compression ratio vs. speed for your specific hardware.
-   **Workflow Automation**: 
    -   Optional **Source Deletion** after successful compression.
    -   Configurable **Detailed Logging** (saved to `zarc.log` in the application directory).
-   **Data Integrity**: BLAKE3 hashing for fast and secure file verification.

### 🚀 Getting Started

#### ZARC Studio (Desktop)
1.  Navigate to `zarc-desktop/`.
2.  Install dependencies: `npm install`.
3.  Run in dev mode: `npm run tauri dev` or build: `npm run tauri build`.

#### ZARC CLI (Python)
1.  Install dependencies: `pip install zstandard cryptography typer rich`.
2.  Run the interactive UI: `python zstd.py`
3.  Or use CLI commands:
    ```bash
    # Compress with level 12, logging enabled, and delete source after success
    python zstd.py --log compress /path/to/source --level 12 --delete-source
    ```

---

## 中文

ZARC (Z-Archive Nexus) 是一款兼顾极致性能与高安全性的现代化存档工具集。它将 **Zstandard (zstd)** 的高速压缩能力与 **AES-256-GCM/XChaCha20Poly1305** 坚固加密相结合，为数据存储与传输提供可靠方案。

本项目提供两种交互方式：
1.  **ZARC Studio (桌面端)**: 基于 Tauri、Vite 和 Rust 构建的跨平台图形界面应用。
2.  **ZARC CLI/TUI (zstd.py)**: 为终端高级用户准备的多功能 Python 脚本。

### ✨ 核心特性

-   **极致压缩**: 基于 Zstandard 算法，支持 22 级压缩等级及多线程并行处理。
-   **安全至上**:
    -   **命令行版**: AES-256-GCM 分块加密。
    -   **桌面版**: XChaCha20Poly1305 认证加密，辅以 Argon2id 密钥派生。
-   **智能存档**: 支持分卷压缩（自动切分大文件）。
-   **性能基准测试**: 内置工具可针对您的硬件环境分析压缩率与吞吐量的平衡点。
-   **自动化工作流**:
    -   可选**压缩后自动删除源文件**。
    -   可配置的**详细日志记录**（默认保存于程序同级目录下的 `zarc.log`）。
-   **数据完整性**: 使用 BLAKE3 哈希算法进行极速、安全的校验。

### 🚀 快速入门

#### ZARC Studio (桌面端)
1.  进入 `zarc-desktop/` 目录。
2.  安装依赖: `npm install`。
3.  启动开发模式: `npm run tauri dev` 或执行构建: `npm run tauri build`。

#### ZARC CLI (Python)
1.  安装依赖: `pip install zstandard cryptography typer rich`。
2.  启动交互式界面: `python zstd.py`。
3.  或直接使用命令行指令:
    ```bash
    # 使用等级 12 压缩，开启日志，并在成功后删除源文件
    python zstd.py --log compress /path/to/source --level 12 --delete-source
    ```

---

## 🛠️ Project Structure / 项目结构

-   `zstd.py`: Python CLI/TUI 核心脚本。
-   `zarc-desktop/`: Tauri 桌面端工程目录。
    -   `src/`: 前端 (TypeScript + Vite) 源码。
    -   `src-tauri/`: 后端 (Rust) 逻辑与底层压缩实现。
-   `.github/workflows/`: 自动化发布流程。

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.
本项目基于 MIT 协议分发。详情请参阅 `LICENSE`。
