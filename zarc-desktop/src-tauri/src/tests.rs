use super::*;
use std::collections::BTreeMap;

fn deterministic_bytes(size: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 131 + 17) % 251) as u8).collect()
}

fn pseudo_random_bytes(size: usize) -> Vec<u8> {
    let mut state = 0x1234_5678_9abc_def0_u64;
    let mut bytes = Vec::with_capacity(size);
    for _ in 0..size {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bytes.push((state & 0xff) as u8);
    }
    bytes
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, bytes).expect("write file");
}

fn collect_file_map(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("strip prefix")
            .to_string_lossy()
            .replace('\\', "/");

        map.insert(rel, fs::read(entry.path()).expect("read file"));
    }
    map
}

fn assert_dirs_equal(expected: &Path, actual: &Path) {
    assert_eq!(collect_file_map(expected), collect_file_map(actual));
}

fn archive_request(
    source: &Path,
    output: &Path,
    level: i32,
    include_root_dir: bool,
    password: Option<&str>,
    split_size_mib: Option<u64>,
) -> CompressRequest {
    CompressRequest {
        source_path: path_to_string(source),
        output_path: Some(path_to_string(output)),
        output_kind: Some(OutputKind::Archive),
        level: Some(level),
        include_root_dir: Some(include_root_dir),
        password: password.map(str::to_string),
        split_size_mib,
        enable_logging: Some(false),
        delete_source_after: Some(false),
        threads: Some(1),
    }
}

#[test]
fn encrypted_roundtrip_file_sizes_and_types() {
    let sizes = [
        0_usize,
        1,
        31,
        4 * 1024,
        ENC_CHUNK_SIZE - 1,
        ENC_CHUNK_SIZE,
        ENC_CHUNK_SIZE + 1,
        ENC_CHUNK_SIZE * 3 + 123,
    ];

    for (idx, size) in sizes.into_iter().enumerate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join(format!("data_{idx}.bin"));
        let archive = temp.path().join(format!("data_{idx}.zst.enc"));
        let output = temp.path().join(format!("out_{idx}.bin"));

        let payload = deterministic_bytes(size);
        write_file(&source, &payload);

        compress_archive_sync(
            archive_request(&source, &archive, 8, true, Some("Strong#Pass123"), None),
            None,
            None,
        )
        .expect("compress encrypted");

        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&output)),
                password: Some("Strong#Pass123".to_string()),
            },
            None,
            None,
        )
        .expect("decompress encrypted");

        let restored = fs::read(&output).expect("read output");
        assert_eq!(restored, payload, "size={size}");
    }
}

#[test]
fn encrypted_roundtrip_directory_include_root_variants() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_dir = temp.path().join("project_src");
    fs::create_dir_all(&source_dir).expect("create source dir");

    write_file(&source_dir.join("empty.txt"), b"");
    write_file(&source_dir.join("plain.txt"), b"hello encrypted world");
    write_file(&source_dir.join("config.json"), br#"{"k":1,"v":"x"}"#);
    write_file(
        &source_dir.join("nested/bin.dat"),
        &deterministic_bytes(131_072),
    );
    write_file(
        &source_dir.join("nested/unicode/中文-emoji.txt"),
        "你好, encryption ✓".as_bytes(),
    );
    write_file(
        &source_dir.join("nested/huge/chunk.bin"),
        &deterministic_bytes(ENC_CHUNK_SIZE * 2 + 77),
    );

    for include_root in [true, false] {
        let archive = temp.path().join(format!("dir_{include_root}.tar.zst.enc"));
        let out_dir = temp.path().join(format!("out_{include_root}"));

        compress_archive_sync(
            archive_request(
                &source_dir,
                &archive,
                6,
                include_root,
                Some("Dir#Secure987"),
                None,
            ),
            None,
            None,
        )
        .expect("compress dir encrypted");

        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&out_dir)),
                password: Some("Dir#Secure987".to_string()),
            },
            None,
            None,
        )
        .expect("decompress dir encrypted");

        let actual_root = if include_root {
            out_dir.join("project_src")
        } else {
            out_dir.clone()
        };
        assert_dirs_equal(&source_dir, &actual_root);
    }
}

#[test]
fn encrypted_archive_rejects_wrong_password() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source = temp.path().join("secret.bin");
    let archive = temp.path().join("secret.zst.enc");
    let output = temp.path().join("secret.out");

    write_file(&source, &deterministic_bytes(8192));

    compress_archive_sync(
        archive_request(&source, &archive, 5, true, Some("CorrectPassword"), None),
        None,
        None,
    )
    .expect("compress encrypted");

    let err = decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: Some(path_to_string(&output)),
            password: Some("WrongPassword".to_string()),
        },
        None,
        None,
    )
    .expect_err("wrong password must fail");

    assert!(!err.to_string().is_empty());
}

#[test]
fn plain_roundtrip_still_works_after_encryption_feature() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source = temp.path().join("plain.bin");
    let archive = temp.path().join("plain.bin.zst");
    let output = temp.path().join("plain.out");

    let payload = deterministic_bytes(2 * 1024 * 1024 + 13);
    write_file(&source, &payload);

    compress_archive_sync(
        archive_request(&source, &archive, 9, true, None, None),
        None,
        None,
    )
    .expect("compress plain");

    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect("decompress plain");

    assert_eq!(fs::read(output).expect("read output"), payload);
}

#[test]
fn literal_windows_reserved_name_roundtrips() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_dir = temp.path().join("reserved_source");
    fs::create_dir_all(&source_dir).expect("create source dir");

    let reserved_source = source_dir.join("NUL");
    let reserved_source_access = fs_access_path(&reserved_source).expect("source access path");
    fs::write(&reserved_source_access, b"literal NUL filename").expect("write NUL file");

    let archive = temp.path().join("reserved.tar.zst");
    compress_archive_sync(
        archive_request(&source_dir, &archive, 3, false, None, None),
        None,
        None,
    )
    .expect("compress reserved name");

    let output = temp.path().join("reserved_output");
    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect("decompress reserved name");

    let reserved_output = output.join("NUL");
    let reserved_output_access = fs_access_path(&reserved_output).expect("output access path");
    assert_eq!(
        fs::read(&reserved_output_access).expect("read restored NUL file"),
        b"literal NUL filename"
    );

    // Windows 上常规递归清理无法寻址字面 NUL 条目，需经 verbatim 命名空间删除特殊文件。
    // On Windows, ordinary recursive cleanup cannot address a literal NUL
    // entry, so remove the special files through the verbatim namespace.
    fs::remove_file(&reserved_source_access).expect("remove source NUL file");
    fs::remove_file(&reserved_output_access).expect("remove restored NUL file");
}

#[test]
fn split_archive_reports_first_volume_and_roundtrips() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source = temp.path().join("split.bin");
    let archive = temp.path().join("split.bin.zst");
    let output = temp.path().join("split.out");

    let payload = pseudo_random_bytes(3 * 1024 * 1024 + 113);
    write_file(&source, &payload);

    let report = compress_archive_sync(
        archive_request(&source, &archive, 1, true, None, Some(1)),
        None,
        None,
    )
    .expect("compress split archive");

    let first_volume = temp.path().join("split.bin.zst.001");
    let second_volume = temp.path().join("split.bin.zst.002");
    assert_eq!(report.output_path, path_to_string(&first_volume));
    assert!(first_volume.exists());
    assert!(second_volume.exists());
    assert_eq!(
        report.blake3_hash,
        Some(
            calculate_archive_hash(&first_volume, detect_archive_meta(&first_volume).unwrap())
                .unwrap()
        )
    );

    let volumes = existing_volume_paths(&archive).expect("list split volumes");
    assert!(volumes.len() > 1);
    let expected_size: u64 = volumes
        .iter()
        .map(|path| fs::metadata(path).unwrap().len())
        .sum();
    assert_eq!(report.output_bytes, expected_size);

    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&first_volume),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect("decompress split archive");

    assert_eq!(fs::read(output).expect("read output"), payload);
}

#[test]
fn benchmark_compression_returns_recommendation() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("blob.bin");
    write_file(&source_path, &deterministic_bytes(2 * 1024 * 1024));

    let report = benchmark_compression_sync(
        BenchmarkRequest {
            source_path: path_to_string(&source_path),
            min_level: Some(1),
            max_level: Some(4),
            iterations: Some(1),
            sample_size_mib: Some(16),
            threads: Some(1),
        },
        None,
    )
    .expect("benchmark");

    assert_eq!(report.results.len(), 4);
    assert!((1..=4).contains(&report.recommended_level));
    assert!(report.sample_bytes > 0);
}

// --- 加密层 Encryption layer ---

#[test]
fn legacy_zenc0001_archives_still_decrypt() {
    // 前向兼容防护：`ZENC0002` 把 Argon2 参数写入头。v1 归档无此块，读取方必须回退 `LEGACY_KDF` 并在正确偏移读取密文。
    // Forward compatibility guard: `ZENC0002` writes its Argon2 parameters
    // into the header. A v1 archive has no such block, so the reader must
    // fall back to `LEGACY_KDF` and read the ciphertext at the right offset.
    let payload = pseudo_random_bytes(600 * 1024);
    let password = "legacy-pw";

    let mut encrypted = Vec::new();
    {
        let mut salt = [0_u8; ENC_SALT_LEN];
        let mut nonce_prefix = [0_u8; ENC_NONCE_PREFIX_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_prefix);

        encrypted.extend_from_slice(ENC_MAGIC_V1);
        encrypted.extend_from_slice(&salt);
        encrypted.extend_from_slice(&nonce_prefix);

        let key = derive_encryption_key(password, &salt, LEGACY_KDF).expect("derive");
        let cipher = key.cipher();
        for (index, chunk) in payload.chunks(ENC_CHUNK_SIZE).enumerate() {
            let nonce = make_nonce(nonce_prefix, index as u64);
            let mut buffer = chunk.to_vec();
            cipher
                .encrypt_in_place(XNonce::from_slice(&nonce), &[], &mut buffer)
                .expect("encrypt");
            encrypted.extend_from_slice(&(buffer.len() as u32).to_be_bytes());
            encrypted.extend_from_slice(&buffer);
        }
        encrypted.extend_from_slice(&0_u32.to_be_bytes());
    }

    let mut reader =
        EncryptedReader::new(io::Cursor::new(encrypted), password).expect("open v1 reader");
    let mut decrypted = Vec::new();
    reader.read_to_end(&mut decrypted).expect("read v1 payload");
    assert_eq!(decrypted, payload);
}

#[test]
fn zenc0002_header_carries_kdf_params() {
    let mut sink = Vec::new();
    {
        let mut writer = EncryptedWriter::new(&mut sink, "pw").expect("writer");
        writer.write_all(b"hello").expect("write");
        writer.finish().expect("finish");
    }

    assert_eq!(&sink[..8], ENC_MAGIC_V2);
    let kdf_offset = 8 + ENC_SALT_LEN + ENC_NONCE_PREFIX_LEN;
    let mut raw = [0_u8; 12];
    raw.copy_from_slice(&sink[kdf_offset..kdf_offset + 12]);
    assert_eq!(KdfParams::from_bytes(raw).expect("parse kdf"), CURRENT_KDF);
}

#[test]
fn hostile_kdf_params_are_rejected() {
    // KDF 参数存入头意味着恶意归档可让 `derive` 分配荒谬内存；边界检查必须在 Argon2 之前拒绝。
    // Storing the KDF parameters in the header means a crafted archive could
    // ask us to allocate an absurd amount of memory during `derive`. The
    // bounds check must reject it before Argon2 ever sees it.
    let huge = KdfParams {
        m_cost_kib: u32::MAX,
        t_cost: 2,
        parallelism: 1,
    };
    assert!(KdfParams::from_bytes(huge.to_bytes()).is_err());

    let zeroed = KdfParams {
        m_cost_kib: 0,
        t_cost: 0,
        parallelism: 0,
    };
    assert!(KdfParams::from_bytes(zeroed.to_bytes()).is_err());
}

#[test]
fn absurd_chunk_length_is_rejected_without_allocating() {
    // 4 字节长度前缀完全受攻击者控制，必须在分配前校验边界。
    // A 4-byte length prefix is fully attacker-controlled and must be
    // bounded before allocation.
    let mut sink = Vec::new();
    {
        let mut writer = EncryptedWriter::new(&mut sink, "pw").expect("writer");
        writer.write_all(b"payload").expect("write");
        writer.finish().expect("finish");
    }

    let header_len = 8 + ENC_SALT_LEN + ENC_NONCE_PREFIX_LEN + 12;
    let mut forged = sink[..header_len].to_vec();
    forged.extend_from_slice(&u32::MAX.to_be_bytes());
    forged.extend_from_slice(&[0_u8; 64]);

    let mut reader = EncryptedReader::new(io::Cursor::new(forged), "pw").expect("reader");
    let mut out = Vec::new();
    let err = reader.read_to_end(&mut out).expect_err("must reject");
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("加密分块长度非法"));
}

// --- 输出保护 Output guard ---

#[test]
fn compress_refuses_to_overwrite_existing_output() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("input.bin");
    write_file(&source_path, &deterministic_bytes(4096));

    let output = temp.path().join("precious.tar.zst");
    write_file(&output, b"do not destroy me");

    let err = compress_archive_sync(
        archive_request(&source_path, &output, 3, true, None, None),
        None,
        None,
    )
    .expect_err("must refuse");

    assert!(err.to_string().contains("输出路径已存在"));
    assert_eq!(
        fs::read(&output).expect("read output"),
        b"do not destroy me"
    );
}

#[test]
fn split_compress_refuses_to_overwrite_existing_volume() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("input.bin");
    write_file(&source_path, &pseudo_random_bytes(512 * 1024));

    let output = temp.path().join("bundle.tar.zst");
    let stale_second = temp.path().join("bundle.tar.zst.002");
    write_file(&stale_second, b"stale volume from an unrelated archive");

    let err = compress_archive_sync(
        archive_request(&source_path, &output, 3, true, None, Some(1)),
        None,
        None,
    )
    .expect_err("must refuse");

    assert!(err.to_string().contains("输出路径已存在"));
    assert!(stale_second.exists());
}

// --- 分卷链 Volume chain ---

#[test]
fn volume_base_path_strips_any_index() {
    assert_eq!(
        volume_base_path(Path::new("/tmp/a.tar.zst.003")),
        PathBuf::from("/tmp/a.tar.zst")
    );
    assert_eq!(
        volume_base_path(Path::new("/tmp/a.tar.zst.001")),
        PathBuf::from("/tmp/a.tar.zst")
    );
    // 非分卷后缀：保持原样 / Not a volume suffix: must be left alone.
    assert_eq!(
        volume_base_path(Path::new("/tmp/a.tar.zst")),
        PathBuf::from("/tmp/a.tar.zst")
    );
}

#[test]
fn decompress_accepts_any_volume_and_detects_gaps() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("payload.bin");
    let payload = pseudo_random_bytes(3 * 1024 * 1024);
    write_file(&source_path, &payload);

    let archive = temp.path().join("payload.bin.zst");
    compress_archive_sync(
        archive_request(&source_path, &archive, 3, true, None, Some(1)),
        None,
        None,
    )
    .expect("compress split");

    let volumes = existing_volume_paths(&archive).expect("volumes");
    assert!(
        volumes.len() >= 3,
        "expected several volumes, got {}",
        volumes.len()
    );

    let output = temp.path().join("restored.bin");
    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&volumes[2]),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect("decompress from middle volume");
    assert_eq!(fs::read(&output).expect("read output"), payload);

    // 链上的空洞必须点名报出，不能静默截断 / A hole in the chain must be named, not silently truncated.
    fs::remove_file(&volumes[1]).expect("remove volume 2");
    let err = decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&volumes[0]),
            output_path: Some(path_to_string(&temp.path().join("restored2.bin"))),
            password: None,
        },
        None,
        None,
    )
    .expect_err("gap must be detected");
    assert!(
        err.to_string().contains("缺少第 2 卷"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn split_archive_hash_covers_every_volume() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("payload.bin");
    write_file(&source_path, &pseudo_random_bytes(3 * 1024 * 1024));

    let archive = temp.path().join("payload.bin.zst");
    compress_archive_sync(
        archive_request(&source_path, &archive, 3, true, None, Some(1)),
        None,
        None,
    )
    .expect("compress split");

    let volumes = existing_volume_paths(&archive).expect("volumes");
    assert!(volumes.len() >= 2);

    let meta = detect_archive_meta(&volumes[0]).expect("meta");
    let whole = calculate_archive_hash(&volumes[0], meta).expect("hash");
    let first_only = calculate_file_hash(&volumes[0]).expect("hash first");
    assert_ne!(
        whole, first_only,
        "hash must cover the whole chain, not just .001"
    );

    // 通过拼接各分卷独立复现 / Reproduce it independently by concatenating the volumes.
    let mut expected = blake3::Hasher::new();
    for volume in &volumes {
        expected.update(&fs::read(volume).expect("read volume"));
    }
    assert_eq!(whole, expected.finalize().to_hex().to_string());
}

#[test]
fn multi_volume_writer_rotates_without_degrading_to_one_byte_writes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let base = temp.path().join("chunked.bin");
    let payload = deterministic_bytes(5 * 1024 * 1024);

    {
        let mut writer =
            MultiVolumeWriter::new(base.clone(), 1, Arc::new(Mutex::new(blake3::Hasher::new())));
        writer.write_all(&payload).expect("write");
        writer.flush().expect("flush");
    }

    let volumes = existing_volume_paths(&base).expect("volumes");
    assert_eq!(volumes.len(), 5);
    for volume in &volumes {
        assert_eq!(fs::metadata(volume).expect("meta").len(), 1024 * 1024);
    }

    let mut rejoined = Vec::new();
    for volume in &volumes {
        rejoined.extend_from_slice(&fs::read(volume).expect("read"));
    }
    assert_eq!(rejoined, payload);
}

#[test]
fn multi_volume_writer_survives_absurd_volume_size() {
    // `volume_limit_mib * 1024 * 1024` 以饱和运算处理，超大上限等价于“单个超大分卷”。
    // `volume_limit_mib * 1024 * 1024` saturates, so an absurd limit is
    // equivalent to "one huge volume".
    let temp = tempfile::tempdir().expect("temp dir");
    let base = temp.path().join("huge.bin");
    let payload = deterministic_bytes(64 * 1024);

    {
        let mut writer = MultiVolumeWriter::new(
            base.clone(),
            u64::MAX,
            Arc::new(Mutex::new(blake3::Hasher::new())),
        );
        writer.write_all(&payload).expect("write");
        writer.flush().expect("flush");
    }

    let volumes = existing_volume_paths(&base).expect("volumes");
    assert_eq!(volumes.len(), 1);
    assert_eq!(fs::read(&volumes[0]).expect("read"), payload);
}

// --- 事务性解压 Transactional extraction ---

#[test]
fn failed_extraction_leaves_no_staging_directory() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source = temp.path().join("tree");
    write_file(&source.join("a.bin"), &pseudo_random_bytes(512 * 1024));
    write_file(
        &source.join("nested/b.bin"),
        &pseudo_random_bytes(512 * 1024),
    );

    let archive = temp.path().join("tree.tar.zst");
    compress_archive_sync(
        archive_request(&source, &archive, 3, true, None, None),
        None,
        None,
    )
    .expect("compress");

    // 中途截断，使 tar 解压半途失败 / Truncate mid-stream so tar extraction fails partway through.
    let raw = fs::read(&archive).expect("read archive");
    let broken = temp.path().join("broken.tar.zst");
    fs::write(&broken, &raw[..raw.len() / 2]).expect("write truncated");

    let output_dir = temp.path().join("dest");
    fs::create_dir_all(&output_dir).expect("create dest");
    let output = output_dir.join("tree_extracted");

    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&broken),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect_err("truncated archive must fail");

    assert!(!output.exists(), "output must not be created on failure");
    let leftovers: Vec<_> = fs::read_dir(&output_dir)
        .expect("read dest")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        leftovers.is_empty(),
        "staging path leaked into the output directory: {leftovers:?}"
    );
}

#[test]
fn concurrent_extractions_do_not_share_a_staging_path() {
    let temp = tempfile::tempdir().expect("temp dir");
    let parent = temp.path();
    let first = unique_temp_path(parent).expect("first");
    let second = unique_temp_path(parent).expect("second");
    assert_ne!(
        first, second,
        "two extractions in one directory must not collide"
    );
}

// --- 预览 Preview ---

#[test]
fn preview_reports_real_size_and_rejects_wrong_password() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("blob.bin");
    let payload = pseudo_random_bytes(700 * 1024);
    write_file(&source_path, &payload);

    let archive = temp.path().join("blob.bin.zst.enc");
    compress_archive_sync(
        archive_request(&source_path, &archive, 3, true, Some("pw123"), None),
        None,
        None,
    )
    .expect("compress");

    let report = list_archive_content_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: None,
            password: Some("pw123".to_string()),
        },
        None,
        None,
    )
    .expect("preview");

    assert_eq!(report.total_files, 1);
    assert_eq!(report.uncompressed_size, payload.len() as u64);
    assert_eq!(report.entries[0].path, "blob.bin");

    let err = list_archive_content_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: None,
            password: Some("wrong".to_string()),
        },
        None,
        None,
    )
    .expect_err("wrong password must fail");
    // 只显示最外层上下文；`{:#}` 遍历整条链 / `to_string()` shows only the outermost context; `{:#}` walks the chain.
    let chain = format!("{err:#}");
    assert!(
        chain.contains("解密失败") || chain.contains("密码"),
        "unexpected error: {chain}"
    );
}

#[test]
fn preview_hash_matches_full_volume_chain() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("payload.bin");
    write_file(&source_path, &pseudo_random_bytes(3 * 1024 * 1024));

    let archive = temp.path().join("payload.bin.zst");
    compress_archive_sync(
        archive_request(&source_path, &archive, 3, true, None, Some(1)),
        None,
        None,
    )
    .expect("compress split");

    let volumes = existing_volume_paths(&archive).expect("volumes");
    let report = list_archive_content_sync(
        DecompressRequest {
            archive_path: path_to_string(&volumes[0]),
            output_path: None,
            password: None,
        },
        None,
        None,
    )
    .expect("preview split");

    let meta = detect_archive_meta(&volumes[0]).expect("meta");
    assert_eq!(
        report.hash,
        calculate_archive_hash(&volumes[0], meta).expect("hash")
    );
}

// --- 符号链接 Symlinks ---

#[cfg(unix)]
#[test]
fn directory_roundtrip_preserves_symlinks() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source = temp.path().join("tree");
    write_file(&source.join("real.txt"), b"real content");
    std::os::unix::fs::symlink("real.txt", source.join("link.txt")).expect("symlink");

    let archive = temp.path().join("tree.tar.zst");
    compress_archive_sync(
        archive_request(&source, &archive, 3, false, None, None),
        None,
        None,
    )
    .expect("compress");

    let output = temp.path().join("restored");
    decompress_archive_sync(
        DecompressRequest {
            archive_path: path_to_string(&archive),
            output_path: Some(path_to_string(&output)),
            password: None,
        },
        None,
        None,
    )
    .expect("decompress");

    let restored_link = output.join("link.txt");
    let link_meta = fs::symlink_metadata(&restored_link).expect("symlink metadata");
    assert!(
        link_meta.file_type().is_symlink(),
        "symlink was dropped or materialised as a regular file"
    );
    assert_eq!(
        fs::read_link(&restored_link).expect("read_link"),
        PathBuf::from("real.txt")
    );
}

// --- 基准测试 Benchmark ---

#[test]
fn benchmark_sample_fills_requested_size() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("blob.bin");
    write_file(&source_path, &deterministic_bytes(9 * 1024 * 1024));

    // 9 MiB 文件上设 3 MiB 上限：从头/中/尾取样，结果应接近上限而非短读的量。
    // 3 MiB cap over a 9 MiB file: sampled from head/middle/tail, so we
    // should get very close to the cap rather than a short read's worth.
    let sample = load_benchmark_sample(&source_path, 3 * 1024 * 1024).expect("sample");
    assert_eq!(sample.len(), 3 * (1024 * 1024));
}

#[test]
fn benchmark_honours_abort_between_chunks() {
    let state = AppState::new();
    state.request_abort();
    let err = compress_to_count(&pseudo_random_bytes(8 * 1024 * 1024), 3, 1, Some(&state))
        .expect_err("must abort");
    assert!(err.to_string().contains("终止"));
}
