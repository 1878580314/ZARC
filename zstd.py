# -*- coding: utf-8 -*-
"""
Z-Archive Nexus (Reforged)
=============================================================
A high-performance, secure, chunk-based streaming archiver.
Optimized by: Code Expert
Stack: Typer, Rich, Zstandard, Cryptography
"""

from __future__ import annotations

import io
import logging
import os
import struct
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import IO, BinaryIO, Optional, cast

import typer
import zstandard as zstd
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from rich.console import Console
from rich.filesize import decimal
from rich.panel import Panel
from rich.progress import (
    BarColumn,
    DownloadColumn,
    Progress,
    SpinnerColumn,
    TextColumn,
    TimeElapsedColumn,
    TransferSpeedColumn,
)
from rich.prompt import Confirm, IntPrompt, Prompt
from rich.table import Table
from rich.theme import Theme

# --- Configuration & Constants ---
APP_NAME = "Z-Archive Nexus"
VERSION = "2.0.1"

def setup_logging(enabled: bool):
    if not enabled:
        logging.getLogger().addHandler(logging.NullHandler())
        return
    
    log_file = Path(__file__).parent / "zarc.log"
    logging.basicConfig(
        filename=str(log_file),
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
BUFFER_SIZE = 1 * 1024 * 1024  # 1MB I/O Buffer
CHUNK_SIZE = 64 * 1024  # 64KB Encryption Chunk
SALT_SIZE = 16
NONCE_SIZE = 12
TAG_SIZE = 16
KEY_SIZE = 32  # AES-256
PBKDF2_ITERATIONS = 600_000
MAGIC_HEADER = b"ZARCv2"  # Versioned Header

# --- UI Theme ---
theme = Theme(
    {
        "info": "cyan",
        "warning": "yellow",
        "error": "bold red",
        "success": "bold green",
        "highlight": "bold magenta",
        "muted": "dim white",
        "panel.border": "blue",
    }
)
console = Console(theme=theme)
app = typer.Typer(help=f"{APP_NAME}: 极速安全压缩工具", add_completion=False)


# --- Cryptography Engine (Chunked GCM) ---


def derive_key(password: str, salt: bytes) -> bytes:
    """Derive a 32-byte key using PBKDF2HMAC-SHA256."""
    kdf = PBKDF2HMAC(
        algorithm=hashes.SHA256(),
        length=KEY_SIZE,
        salt=salt,
        iterations=PBKDF2_ITERATIONS,
    )
    return kdf.derive(password.encode("utf-8"))


class ChunkedAESWriter(io.BufferedIOBase):
    """
    Encrypts data in 64KB chunks using AES-256-GCM.
    Format: [Size(4B)][Nonce(12B)][Ciphertext][Tag(16B)]
    """
    # ... (existing implementation remains same) ...

class MultiVolumeWriter(io.BufferedIOBase):
    """Splits output into multiple files (.001, .002, ...)"""
    def __init__(self, base_path: Path, volume_limit: int):
        self.base_path = base_path
        self.volume_limit = volume_limit
        self.current_index = 1
        self.bytes_written_in_vol = 0
        self.current_file: Optional[BinaryIO] = None

    def _get_path(self, index: int) -> Path:
        return self.base_path.with_suffix(f"{self.base_path.suffix}.{index:03d}")

    def _ensure_file(self):
        if self.current_file is None:
            path = self._get_path(self.current_index)
            self.current_file = open(path, "wb")
        return self.current_file

    def write(self, b: bytes) -> int:
        written = 0
        while written < len(b):
            remaining_in_vol = self.volume_limit - self.bytes_written_in_vol
            if remaining_in_vol <= 0:
                if self.current_file:
                    self.current_file.close()
                self.current_index += 1
                self.current_file = None
                self.bytes_written_in_vol = 0
                remaining_in_vol = self.volume_limit

            f = self._ensure_file()
            chunk = b[written : written + remaining_in_vol]
            n = f.write(chunk)
            written += n
            self.bytes_written_in_vol += n
        return written

    def flush(self):
        if self.current_file:
            self.current_file.flush()

    def close(self):
        if self.current_file:
            self.current_file.close()
            self.current_file = None
        super().close()

class ArchiveEngine:
    """Core processing engine ensuring resource safety."""
    # ... (init and helpers) ...

    def run_compress(self, source: Path, level: int, delete_source: bool = False, 
                     split_size_mib: int = 0, include_root: bool = True):
        if not source.exists():
            self.console.print(f"[error]路径不存在: {source}[/error]")
            return

        logging.info(f"开始压缩任务: {source} (等级: {level}, 分卷: {split_size_mib}MiB)")

        # Prepare Inputs
        password = self._get_password(confirm=True)
        ext = ".tar.zst" if source.is_dir() else ".zst"
        if password:
            ext += ".enc"
        dest = source.with_suffix(ext)

        # Calculate Size
        total_size = 0
        if source.is_file():
            total_size = source.stat().st_size
        else:
            with self.console.status("[bold cyan]正在扫描文件结构...", spinner="dots"):
                total_size = sum(f.stat().st_size for f in source.rglob("*") if f.is_file())

        stats = JobStats(source_size=total_size, start_time=os.times().elapsed)
        self.console.rule("[bold]📦 压缩任务[/bold]")

        progress = self._create_progress()
        task_id = progress.add_task("Processing", total=total_size)

        try:
            with progress:
                # Setup Multi-volume or Single file output
                out_base: io.BufferedIOBase
                if split_size_mib > 0:
                    out_base = MultiVolumeWriter(dest, split_size_mib * 1024 * 1024)
                else:
                    out_base = cast(io.BufferedIOBase, open(dest, "wb"))

                with out_base as f_out:
                    output_stream: IO[bytes] = f_out
                    enc_wrapper = None
                    if password:
                        enc_wrapper = ChunkedAESWriter(f_out, password)
                        output_stream = cast(IO[bytes], enc_wrapper)

                    cctx = zstd.ZstdCompressor(level=level, threads=-1)
                    with cctx.stream_writer(output_stream) as zstd_writer:
                        if source.is_dir():
                            with tarfile.open(fileobj=zstd_writer, mode="w|") as tar:
                                for file_path in source.rglob("*"):
                                    if file_path.is_file():
                                        arcname = file_path.relative_to(source.parent if include_root else source)
                                        tar.add(file_path, arcname=arcname)
                                        progress.update(task_id, advance=file_path.stat().st_size)
                        else:
                            with open(source, "rb") as f_in:
                                while chunk := f_in.read(BUFFER_SIZE):
                                    zstd_writer.write(chunk)
                                    progress.update(task_id, advance=len(chunk))
                    
                    if enc_wrapper:
                        enc_wrapper.close()

            stats.end_time = os.times().elapsed
            stats.final_size = dest.stat().st_size if split_size_mib == 0 else 0 # Simplified
            self._show_summary(stats, True, dest)
            
            if delete_source:
                if source.is_dir():
                    import shutil
                    shutil.rmtree(source)
                else:
                    source.unlink()

        except Exception as e:
            logging.error(f"失败: {e}")
            self.console.print(f"[error]💥 错误: {e}[/error]")

    def run_benchmark(self, source: Path, min_level: int, max_level: int, iterations: int, sample_mib: int):
        self.console.rule("[bold magenta]⚡ 性能基准测试[/bold magenta]")
        
        # Load sample
        sample_limit = sample_mib * 1024 * 1024
        sample = bytearray()
        if source.is_file():
            with open(source, "rb") as f:
                sample = f.read(sample_limit)
        else:
            for f_path in source.rglob("*"):
                if f_path.is_file() and len(sample) < sample_limit:
                    with open(f_path, "rb") as f:
                        sample.extend(f.read(1024 * 1024))
        
        if not sample:
            self.console.print("[error]无法获取有效样本[/error]")
            return

        table = Table(title=f"测试源: {source.name} (样本: {decimal(len(sample))})")
        table.add_column("等级", justify="center", style="cyan")
        table.add_column("吞吐量", justify="right", style="green")
        table.add_column("压缩率", justify="right", style="magenta")
        table.add_column("耗时", justify="right")

        with self.console.status("[bold yellow]正在进行基准测试...") as status:
            for level in range(min_level, max_level + 1):
                times = []
                compressed_size = 0
                for _ in range(iterations):
                    start = os.times().elapsed
                    cctx = zstd.ZstdCompressor(level=level)
                    compressed = cctx.compress(sample)
                    times.append(os.times().elapsed - start)
                    compressed_size = len(compressed)
                
                avg_time = sum(times) / len(times)
                speed = len(sample) / avg_time / (1024 * 1024) if avg_time > 0 else 0
                ratio = compressed_size / len(sample) * 100
                table.add_row(str(level), f"{speed:.2f} MiB/s", f"{ratio:.2f}%", f"{avg_time*1000:.1f}ms")
        
        self.console.print(table)

    def list_archive(self, source: Path):
        self.console.rule(f"[bold]🔍 预览归档: {source.name}[/bold]")
        # This is simplified: Only works for unencrypted .tar.zst for now in this demo
        # Real implementation would need to handle decryption stream
        try:
            with open(source, "rb") as f_raw:
                dctx = zstd.ZstdDecompressor()
                with dctx.stream_reader(f_raw) as reader:
                    with tarfile.open(fileobj=reader, mode="r|") as tar:
                        table = Table(box=None)
                        table.add_column("名称", style="blue")
                        table.add_column("大小", justify="right")
                        for member in tar:
                            table.add_row(member.name, decimal(member.size))
                        self.console.print(table)
        except Exception as e:
            self.console.print(f"[error]无法预览内容 (可能已加密或非标准格式): {e}[/error]")

    def run_decompress(self, source: Path):
        if not source.exists():
            self.console.print(f"[error]文件不存在: {source}[/error]")
            return

        # Name Deduction
        clean_name = source.name.replace(".enc", "").replace(".zst", "")
        dest_path = source.parent / clean_name.replace(".tar", "")
        is_tar = (
            ".tar" in str(source)
            or source.name.endswith(".tar.zst")
            or source.name.endswith(".tar.zst.enc")
        )

        # Adjust dest_path for tar extraction
        if is_tar:
            dest_path = source.parent / (dest_path.name + "_extracted")

        file_size = source.stat().st_size
        stats = JobStats(source_size=file_size, start_time=os.times().elapsed)

        self.console.rule("[bold]🔓 解压/解密[/bold]")
        self.console.print(
            f"[muted]源:[/muted] {source.name}  [muted]输出:[/muted] {dest_path.name}"
        )

        progress = self._create_progress()
        task_id = progress.add_task("Decrypting & Unpacking", total=file_size)

        try:
            with progress:
                with open(source, "rb") as f_raw:
                    # 1. Detect Encryption via Header
                    header = f_raw.read(len(MAGIC_HEADER))
                    f_raw.seek(0)
                    is_encrypted = header == MAGIC_HEADER

                    password = None
                    if is_encrypted:
                        # Pause progress to ask for password if needed (though CLI usually asks before progress starts)
                        # Since we are inside progress context, printing might break the bar momentarily.
                        # Ideally we ask before, but we didn't know it was encrypted.
                        # Rich handles print/input inside progress somewhat, but it's cleaner to ask.
                        progress.stop()
                        self.console.print("[info]检测到加密文件头[/info]")
                        password = self._get_password(confirm=False)
                        progress.start()

                    # 2. Progress Wrapper
                    class ProgressReader:
                        def __init__(self, stream: BinaryIO):
                            self._stream = stream

                        def read(self, size: int = -1) -> bytes:
                            data = self._stream.read(size)
                            if data:
                                progress.update(task_id, advance=len(data))
                            return data

                        def seek(self, offset: int, whence: int = 0) -> int:
                            return self._stream.seek(offset, whence)

                        def tell(self) -> int:
                            return self._stream.tell()

                        def readable(self) -> bool:
                            return True

                    monitored_stream = cast(BinaryIO, ProgressReader(f_raw))

                    # 3. Decryption Layer
                    input_stream: IO[bytes] = monitored_stream
                    if is_encrypted:
                        if not password:
                            # Should have been asked above
                            raise ValueError("加密文件需要密码")
                        input_stream = cast(
                            IO[bytes], ChunkedAESReader(monitored_stream, password)
                        )

                    # 4. Decompression & Extraction
                    dctx = zstd.ZstdDecompressor()

                    if is_tar:
                        dest_path.mkdir(parents=True, exist_ok=True)
                        with dctx.stream_reader(input_stream) as zstd_reader:
                            # Tarfile stream read
                            with tarfile.open(fileobj=zstd_reader, mode="r|") as tar:
                                tar.extractall(path=dest_path)
                    else:
                        with open(dest_path, "wb") as f_out:
                            dctx.copy_stream(
                                input_stream,
                                f_out,
                                read_size=BUFFER_SIZE,
                                write_size=BUFFER_SIZE,
                            )

            stats.end_time = os.times().elapsed
            self.console.print(
                Panel(
                    f"[bold green]✔ 操作成功完成[/bold green]\n保存至: [underline]{dest_path}[/underline]",
                    border_style="green",
                )
            )

        except InvalidTag:
            self.console.print(
                "\n[error]⛔ 完整性校验失败: 密码错误或数据块被篡改。[/error]"
            )
        except (Exception, KeyboardInterrupt) as e:
            if dest_path.is_dir():
                import shutil
                shutil.rmtree(dest_path, ignore_errors=True)
            else:
                dest_path.unlink(missing_ok=True)
            
            if isinstance(e, KeyboardInterrupt):
                self.console.print("\n[warning]⚠️ 任务被用户终止，已清理残余文件。[/warning]")
            else:
                self.console.print(f"\n[error]💥 错误: {str(e)}[/error]")


# --- CLI Commands ---

engine = ArchiveEngine(console)


@app.command(name="compress")
def cli_compress(
    path: Path = typer.Argument(..., help="Source file or directory", exists=True),
    level: int = typer.Option(
        3, "--level", "-l", min=1, max=22, help="Compression level (1-22)"
    ),
    delete_source: bool = typer.Option(
        False, "--delete-source", "-d", help="Delete source file/directory after success"
    ),
    split: int = typer.Option(
        0, "--split", "-s", help="Split size in MiB (0 for no split)"
    ),
    no_root: bool = typer.Option(
        False, "--no-root", help="Do not include the root directory itself in archive"
    ),
):
    """Create a secure Zstandard archive."""
    engine.run_compress(path, level, delete_source=delete_source, split_size_mib=split, include_root=not no_root)


@app.command(name="extract")
def cli_extract(
    path: Path = typer.Argument(
        ..., help="Archive file (.zst, .enc)", exists=True, dir_okay=False
    ),
):
    """Decompress and decrypt an archive."""
    engine.run_decompress(path)


@app.command(name="list")
def cli_list(
    path: Path = typer.Argument(..., help="Archive to preview", exists=True),
):
    """Preview contents of an unencrypted archive."""
    engine.list_archive(path)


@app.command(name="benchmark")
def cli_benchmark(
    path: Path = typer.Argument(..., help="Source to test", exists=True),
    min_l: int = 1,
    max_l: int = 12,
    iters: int = 2,
    sample: int = 64,
):
    """Run performance benchmark on a source."""
    engine.run_benchmark(path, min_l, max_l, iters, sample)


@app.command()
def ui():
    """Launch the interactive TUI menu."""
    while True:
        console.clear()

        # Header
        console.print(
            Panel.fit(
                f"[bold blue]{APP_NAME}[/bold blue] [dim]v{VERSION}[/dim]\n"
                "[italic cyan]Next-Gen Secure Storage[/italic cyan]",
                border_style="blue",
                padding=(1, 4),
            )
        )

        # Menu
        console.print("[bold white]1.[/bold white] 📦 压缩文件/文件夹")
        console.print("[bold white]2.[/bold white] 🔓 解压/还原")
        console.print("[bold white]3.[/bold white] 🔍 预览内容 (仅限非加密)")
        console.print("[bold white]4.[/bold white] ⚡ 性能测试 (Benchmark)")
        console.print("[bold white]q.[/bold white] 退出")
        console.print("")

        choice = Prompt.ask("选择操作", choices=["1", "2", "3", "4", "q"], default="1")

        if choice == "q":
            console.print("[heading]Goodbye![/heading]")
            break

        if choice == "1":
            target_path = Path(Prompt.ask("输入待压缩路径").strip('"').strip("'"))
            if not target_path.exists(): continue
            level = IntPrompt.ask("压缩等级 (1-22)", default=3)
            split = IntPrompt.ask("分卷大小 (MiB, 0为不分卷)", default=0)
            inc_root = Confirm.ask("是否包含根目录？", default=True)
            del_src = Confirm.ask("压缩成功后是否删除源文件？", default=False)
            engine.run_compress(target_path, level, delete_source=del_src, split_size_mib=split, include_root=inc_root)
        elif choice == "2":
            target_path = Path(Prompt.ask("输入归档文件路径").strip('"').strip("'"))
            if target_path.exists(): engine.run_decompress(target_path)
        elif choice == "3":
            target_path = Path(Prompt.ask("输入归档文件路径").strip('"').strip("'"))
            if target_path.exists(): engine.list_archive(target_path)
        elif choice == "4":
            target_path = Path(Prompt.ask("输入测试源路径").strip('"').strip("'"))
            if target_path.exists():
                min_l = IntPrompt.ask("最小等级", default=1)
                max_l = IntPrompt.ask("最大等级", default=12)
                engine.run_benchmark(target_path, min_l, max_l, 2, 64)

        Prompt.ask("\n[dim]按回车键继续...[/dim]", show_default=False)


@app.callback(invoke_without_command=True)
def main(
    ctx: typer.Context,
    log: bool = typer.Option(False, "--log", help="Enable detailed logging to zarc.log"),
):
    """
    Z-Archive Nexus: 极速安全压缩工具
    Run without arguments to start the Interactive UI.
    """
    setup_logging(log)
    if ctx.invoked_subcommand is None:
        ui()


if __name__ == "__main__":
    app()
