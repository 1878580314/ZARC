/**
 * Translation for strings produced by the Rust backend.
 *
 * Backend error and log messages are authored in Chinese at the source
 * (they live in `lib.rs` / `sfx.rs`), so the frontend maps the known,
 * bounded set to the active UI language instead of duplicating message
 * catalogs in Rust. Anything unrecognized is passed through untouched,
 * so new or chained messages degrade gracefully.
 *
 * Two rule shapes:
 *  - `{ p, en }`: the Chinese prefix `p` maps to English text `en`; the
 *    remainder of the input keeps being scanned, so wrapped error chains
 *    translate piece by piece.
 *  - `{ r, en }`: a sticky regex; `{1}..{n}` in `en` are filled from the
 *    capture groups and scanning resumes after the match.
 */

interface PrefixRule {
  p: string;
  en: string;
}

interface RegexRule {
  r: RegExp;
  en: string;
}

const PREFIX_RULES: PrefixRule[] = [
  { p: '任务线程异常: ', en: 'Task thread failed: ' },
  { p: '路径检查线程异常: ', en: 'Path inspection thread failed: ' },
  { p: '归档文件不存在: ', en: 'Archive not found: ' },
  { p: '输出路径已存在，请自行更改输出名称: ', en: 'Output path already exists; choose a different name: ' },
  { p: '输出路径已存在，为保护数据拒绝覆盖: ', en: 'Output path already exists; refusing to overwrite: ' },
  { p: '输出路径不能覆盖源路径', en: 'Output path must not overwrite the source' },
  { p: '输出路径不能位于待压缩目录内部', en: 'Output path must not be inside the directory being compressed' },
  { p: '解压输出不能覆盖归档文件', en: 'Extraction output must not overwrite the archive' },
  { p: '该归档已加密，请提供解密密码以预览内容', en: 'This archive is encrypted; provide the password to preview its contents' },
  { p: '该归档已加密，请提供解密密码', en: 'This archive is encrypted; provide the password to extract it' },
  { p: '该自解压包已加密，请输入解密密码', en: 'This self-extracting archive is encrypted; enter the password to extract it' },
  { p: '用户已终止任务', en: 'Task aborted by user' },
  { p: '用户已终止测试', en: 'Benchmark aborted by user' },
  { p: '解密失败：密码错误或文件已损坏', en: 'Decryption failed: wrong password or corrupted file' },
  { p: '无效加密文件头，无法识别的归档格式', en: 'Invalid encrypted header: unrecognized archive format' },
  { p: '不支持的文件类型，仅支持 .zst/.tar.zst 及其 .enc 加密版本', en: 'Unsupported file type; only .zst/.tar.zst and their .enc encrypted variants are supported' },
  { p: '未找到分卷归档首卷: ', en: 'First volume of the multi-volume archive not found: ' },
  { p: '未找到分卷输出首卷: ', en: 'First volume of the split output not found: ' },
  { p: '源路径不存在或无法访问: ', en: 'Source path does not exist or is inaccessible: ' },
  { p: '源路径不存在: ', en: 'Source path not found: ' },
  { p: '基准测试样本为空，无法评估压缩等级', en: 'Benchmark sample is empty; cannot evaluate compression levels' },
  { p: '无法创建输出目录: ', en: 'Failed to create output directory: ' },
  { p: '无法创建输出文件: ', en: 'Failed to create output file: ' },
  { p: '无法创建解压目录: ', en: 'Failed to create extraction directory: ' },
  { p: '无法打开归档文件: ', en: 'Failed to open archive: ' },
  { p: '无法打开分卷归档: ', en: 'Failed to open volume: ' },
  { p: '解包归档失败: ', en: 'Failed to unpack archive: ' },
  { p: '创建临时解压目录失败: ', en: 'Failed to create the staging directory: ' },
  { p: '提交解压结果失败: ', en: 'Failed to commit the extraction result: ' },
  { p: '自解压包内的输出名称非法: ', en: 'Invalid output name inside the self-extracting archive: ' },
  { p: '自解压包内的输出名称为空，无法解压', en: 'The output name inside the self-extracting archive is empty' },
  { p: '输出路径不能覆盖当前运行中的程序', en: 'Output path must not overwrite the currently running program' },
  { p: 'Windows 自解压 EXE 暂不支持分卷', en: 'Multi-volume output is not supported for Windows self-extracting EXEs' },
  { p: 'Windows 自解压 EXE 仅能在 Windows 构建环境中生成', en: 'Windows self-extracting EXEs can only be built in a Windows build environment' },
  { p: '无法定位嵌入归档数据', en: 'Failed to locate the embedded archive data' },
  { p: '无法定位当前程序', en: 'Failed to locate the current program' },
  { p: '无法读取归档信息: ', en: 'Failed to read archive metadata: ' },
  { p: '无法读取结果文件信息: ', en: 'Failed to read result file metadata: ' },
  { p: '无法读取文件信息: ', en: 'Failed to read file metadata: ' },
  { p: '无法准备源路径: ', en: 'Failed to prepare source path: ' },
  { p: '创建 zstd 解码器失败', en: 'Failed to create the zstd decoder' },
  { p: '创建 zstd 编码器失败', en: 'Failed to create the zstd encoder' },
  { p: '无法开启 zstd 多线程压缩', en: 'Failed to enable zstd multithreaded compression' },
  { p: '刷新输出文件失败', en: 'Failed to flush the output file' },
  { p: '同步输出文件到磁盘失败', en: 'Failed to sync the output file to disk' },
  { p: '解压读取失败', en: 'Decompression read failed' },
  { p: '读取归档目录失败', en: 'Failed to read the archive listing' },
  { p: '请选择解压目标目录', en: 'Choose an extraction destination' }
];

const REGEX_RULES: RegexRule[] = [
  {
    r: /归档头中的密钥派生参数超出支持范围 \(m=(\d+) KiB, t=(\d+), p=(\d+)\)/y,
    en: 'Key-derivation parameters in the archive header are out of the supported range (m={1} KiB, t={2}, p={3})'
  },
  {
    r: /加密分块长度非法\((\d+) 字节\)，文件已损坏或不是 ZARC 归档/y,
    en: 'Invalid encrypted chunk length ({1} bytes); the file is corrupted or not a ZARC archive'
  },
  {
    r: /分卷归档不完整，缺少第 (\d+) 卷: /y,
    en: 'Incomplete multi-volume archive; missing volume {1}: '
  },
  {
    r: /无法在 (.+) 下分配临时解压路径/y,
    en: 'Failed to allocate a staging path under {1}'
  },
  {
    r: /找不到数据文件 (.+)：请将它与本程序放在同一目录，且不要更改其名称/y,
    en: 'Data file {1} not found: keep it in the same folder as this program and do not rename it'
  },
  {
    r: /数据文件已损坏（过短）: /y,
    en: 'Data file is corrupted (too short): '
  },
  {
    r: /数据文件已损坏（标识不符）: /y,
    en: 'Data file is corrupted (bad magic): '
  },
  {
    r: /基于样本大小约 ([\d.]+) MiB 的快速压缩测试。推荐等级平衡了压缩率与吞吐（权重：率 60%，速度 40%）。/y,
    en: 'Quick compression test on a sample of about {1} MiB. The recommended level balances compression ratio and throughput (weights: ratio 60%, speed 40%).'
  }
];

const hasChinese = /[\u4e00-\u9fff]/;

/**
 * Translate a backend-produced string into the active UI language.
 * Chinese segments that match a known rule are replaced; everything
 * else (paths, numbers, unknown chains) passes through unchanged.
 */
export function translateBackendText(input: string): string {
  if (!hasChinese.test(input)) return input;

  let out = '';
  let rest = input;

  while (rest.length > 0) {
    let matched = false;

    for (const rule of PREFIX_RULES) {
      if (rest.startsWith(rule.p)) {
        out += rule.en;
        rest = rest.slice(rule.p.length);
        matched = true;
        break;
      }
    }
    if (matched) continue;

    for (const rule of REGEX_RULES) {
      rule.r.lastIndex = 0;
      const match = rule.r.exec(rest);
      if (match) {
        out += rule.en.replace(/\{(\d+)\}/g, (_, n: string) => match[Number(n)] ?? '');
        rest = rest.slice(match[0].length);
        matched = true;
        break;
      }
    }
    if (matched) continue;

    out += rest[0];
    rest = rest.slice(1);
  }

  return out;
}
