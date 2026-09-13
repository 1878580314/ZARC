use super::*;

fn tar_bytes() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, "hello.txt", &b"hello"[..]).unwrap();
    builder.into_inner().unwrap()
}

#[test]
fn default_output_strips_volume_and_case_insensitive_suffix_once() {
    for (input, expected) in [("a.zst.001", "a"), ("FILE.ZST", "FILE"),
        ("Tree.TAR.ZST.ENC.002", "Tree"), ("a.zst.zst", "a.zst")] {
        let path = Path::new(input);
        let meta = detect_archive_meta(path).unwrap();
        assert_eq!(default_decompress_name(path, meta).unwrap(), expected);
    }
}

#[test]
fn output_names_match_actual_format_and_selected_folder() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file.txt");
    fs::write(&file, b"data").unwrap();
    for (source, typed, encrypted, expected) in [
        (file.as_path(), "backup", false, "backup.zst"),
        (temp.path(), "backup.zst", true, "backup.tar.zst.enc"),
        (file.as_path(), "BACKUP.TAR.ZST.ENC", false, "BACKUP.zst"),
    ] {
        let output = resolve_compress_output(source, Some(typed), encrypted, OutputKind::Archive).unwrap();
        assert_eq!(output, Path::new(expected));
        assert!(detect_archive_meta(&output).is_ok());
    }
    let archive = Path::new("tree.tar.zst");
    assert_eq!(resolve_decompress_output(archive, detect_archive_meta(archive).unwrap(),
        Some(temp.path().to_str().unwrap())).unwrap(), temp.path().join("tree"));
}

#[test]
fn corrupted_tail_never_commits_tar_output_or_preview() {
    let temp = tempfile::tempdir().unwrap();
    let mut encoded = zstd::encode_all(&tar_bytes()[..], 1).unwrap();
    encoded.extend_from_slice(b"corrupt second frame");
    let output = temp.path().join("out");
    assert!(decompress_reader_transactionally(&encoded[..], ArchiveKind::TarZst, &output, None).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    let meta = detect_archive_meta(Path::new("a.tar.zst")).unwrap();
    assert!(read_archive_listing(&encoded[..], Path::new("a.tar.zst"), meta,
        &mut Vec::new(), &mut 0).is_err());
}

#[test]
fn cancellation_during_tar_file_does_not_retry() {
    struct CancelAfterHeader {
        inner: AbortableReader<io::Cursor<Vec<u8>>>,
        state: AppState,
        calls: Arc<AtomicUsize>,
    }
    impl Read for CancelAfterHeader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.inner.inner.position() >= 512 {
                self.state.request_abort();
                // 有限保护避免回归后测试挂死。 / Bound retries so regressions cannot hang tests.
                if self.calls.fetch_add(1, AtomicOrdering::SeqCst) > 2 {
                    return Err(io::Error::other("test retry limit"));
                }
            }
            self.inner.read(buf)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let reader = CancelAfterHeader {
        inner: AbortableReader::new(io::Cursor::new(tar_bytes()), Some(&state)),
        state, calls: calls.clone(),
    };
    assert!(tar::Archive::new(reader).unpack(temp.path()).is_err());
    assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
}

#[cfg(unix)]
#[test]
fn source_alias_with_uncreated_parent_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let alias = temp.path().join("alias");
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    assert!(validate_compress_paths(&source, &alias.join("new/a.tar.zst"), OutputKind::Archive, None).is_err());
}

#[cfg(unix)]
#[test]
fn unreadable_subtree_aborts_directory_compression() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let locked = source.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(locked.join("important.txt"), b"important").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0)).unwrap();
    // root 可越过权限；此环境无法模拟此失败。 / Root bypasses this permission fixture.
    let inaccessible = fs::read_dir(&locked).is_err();
    let result = compress_directory(&source, &temp.path().join("out.tar.zst"), 1, false,
        None, &ProgressReporter::new(None, "compress", 9), None, None, Some(1));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    if inaccessible { assert!(result.is_err()); }
}

#[test]
fn obsolete_directory_measurement_stops() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("empty")).unwrap();
    let generation = INSPECTION_GENERATION.fetch_add(1, AtomicOrdering::Relaxed);
    assert!(measure_directory(temp.path(), generation).2);
}
