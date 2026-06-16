# Code Review 报告 — Backup Utility

- **审查日期:** 2026-06-16
- **审查范围:** `src/` 全部源代码(`app.rs`, `config.rs`, `backup_entry.rs`, `backup_runner.rs`, `compressor.rs`, `main.rs`, `ui/` 三个模块)
- **审查基线:** commit 对应当前工作区(编译通过,clippy 零警告)
- **审查方式:** 人工逐文件审阅 + 实测验证关键怀疑点

## 实测验证(非臆测)

以下结论均经过实际命令验证,非基于推测:

| 验证项 | 命令 | 结果 |
|---|---|---|
| macOS bsdtar 是否识别 `GZIP` 环境变量 | `GZIP="-9" tar czf ...` vs 无环境变量 | **两个 tar.gz 大小完全相同(均 574 字节)**,证明 macOS bsdtar 忽略 `GZIP` 环境变量,压缩级别静默失效 |
| Windows 是否有 `which` 命令 | — | 无,Windows 用 `where`。当前代码硬编码 `Command::new("which")`,Windows 上必定失败 |

---

## 🔴 严重问题(导致功能错误或破坏需求)

### 1. Windows 上无法检测到任何压缩工具 — 违反"兼容 Windows"需求

- **文件:** `src/compressor.rs:51-57`
- **现象:**
  ```rust
  fn which_command(cmd: &str) -> bool {
      Command::new("which")   // ← Windows 没有 which 命令
          .arg(cmd)
          ...
  }
  ```
- **影响:** Windows 用的是 `where`,没有 `which`。所以在 Windows 上即使用户装了 7z,程序也会报"未找到 7z 命令"。完全违背需求第 5 条"兼容 mac 以及 windows 使用"。
- **建议:** 改用跨平台方案,例如 `which` crate;或在 Windows 上调用 `where`、其他平台调用 `which`。

### 2. tar 的压缩级别完全静默失效

- **文件:** `src/compressor.rs:158`
  ```rust
  cmd.env("GZIP", format!("-{gzip_level}"));
  ```
- **现象:** 实测 macOS 自带的 bsdtar(libarchive):设置 `GZIP="-9"` 与不设置,生成的 tar.gz 大小完全相同(574 字节)。
- **影响:** `GZIP` 环境变量只有 **GNU tar + GNU gzip** 组合才识别。macOS 默认是 bsdtar,Windows 也不一定有 GNU tar。结果:用户在 UI 上选了压缩级别,对 tar 格式**完全无效**,而且没有任何提示。更糟的是,bsdtar 在遇到非自己识别的 GZIP 值时,某些版本还会打印警告。
- **建议:** 改用 `tar -I "gzip -9"`(GNU tar)或明确告知用户 tar 级别受限;或在 UI 上为 tar 格式禁用压缩级别滑块并显示说明。

### 3. 切换压缩工具后级别不重置 → 生成非法参数

- **文件:** `src/compressor.rs` + `src/ui/settings.rs:69-70`
- **现象:** 当用户从 7z(级别 9)切换到 rar(最大级别 5)时,`config.compression_level` 仍是 9。Slider 的 `max` 会变,但**内存里的值不会被 clamp**。于是 `compress_rar` 会执行 `rar a -m9 ...`,而 rar 的合法范围是 `-m0..-m5`,会报错。
- **建议:** 在 ComboBox 选择后立即重置级别:
  ```rust
  if config.compression_level > config.compressor.max_level() {
      config.compression_level = config.compressor.max_level();
  }
  ```

### 4. 密码以明文写入配置文件

- **文件:** `src/config.rs:148` + `src/app.rs:103-116`
- **现象:** `BackupConfig.password: Option<String>` 被 `#[derive(Serialize)]` 序列化,`save_config` 会把它明文写到 `config.json`:
  ```json
  { "password": "用户的真实密码", ... }
  ```
- **影响:** 与计划中"密码字段:默认不存储密码"不符,是明显的安全隐患。配置文件明文落盘,任何能读到该文件的进程都能拿到密码。
- **建议:** 给该字段加 `#[serde(skip)]`,让密码仅在内存中、每次运行手动输入。

### 5. 同一天多次运行会覆盖上一次的备份

- **文件:** `src/backup_runner.rs:159-167`
  ```rust
  let output_filename = config.output_filename_pattern.replace("{date}", &date);
  // → "backup20260616.7z"
  ```
- **影响:** 默认模板只有 `{date}`(YYYYMMDD),第二次运行会**直接覆盖**第一次的备份,没有任何确认或提示。原脚本是手动的所以问题不大,但 GUI 工具用户很容易连续点两次"开始备份"。
- **建议:** 检测输出文件已存在时,在日志里显著告警;或模板默认带时间 `{datetime}`;或在 UI 上对覆盖做二次确认。

---

## 🟠 中等问题(可靠性/一致性缺陷)

### 6. zip 和 rar 完全没有排除项,与 7z 不一致

- **文件:** `src/compressor.rs` 各 `compress_*` 函数对比
  - 7z:有 `.DS_Store`、`Thumbs.db`、`node_modules` 等 5 个排除项 ✅
  - zip:**零排除项** ❌
  - rar:**零排除项** ❌
- **影响:** 用户选了 zip/rar 后,所有垃圾文件会全部进包。`backup_entry.rs` 里的 `should_skip` 已在复制阶段过滤了大部分,但对于 Git 仓库导出的内容和将来可能绕过复制阶段的场景,compressor 层的排除仍是重要的二次防线。
- **建议:** 统一处理各格式的排除项(zip 用 `-x`,rar 用 `-x`,tar 用 `--exclude`)。

### 7. `is_cache_dir` 的路径匹配过于宽泛

- **文件:** `src/backup_entry.rs:107-112`
  ```rust
  path_str.contains(".cache")
  ```
- **影响:** `contains` 匹配整个路径字符串,所以 `/Users/myapp.cache/data`、`/Users/someone.cache_project/x` 都会被误判为缓存目录而跳过。`copilot/history` 同理。
- **建议:** 匹配路径的末尾组件或用分量比较:
  ```rust
  path.file_name().map(|n| n == ".cache").unwrap_or(false)
  ```

### 8. 后台线程 panic 时 UI 永久卡死

- **文件:** `src/app.rs:54-67` + `src/backup_runner.rs`
- **现象:** 如果后台线程 panic,所有 sender 被 drop,`try_recv()` 返回 `Err`(channel 关闭),while 循环不执行,`should_stop` 保持 `false`。于是 `is_running` 永远为 `true`,**"开始备份"按钮永久禁用,状态卡在"正在收集/压缩"**。
- **建议:** 检测 channel 关闭:
  ```rust
  if let Some(rx) = &self.status_receiver {
      match rx.try_recv() {
          Ok(status) => { ... }
          Err(mpsc::TryRecvError::Empty) => {}
          Err(mpsc::TryRecvError::Disconnected) => {
              // 线程已结束但没发 Done/Error → 可能 panic 了
              if self.is_running {
                  self.is_running = false;
                  self.backup_status = BackupStatus::Error("后台任务异常终止".into());
              }
          }
      }
  }
  ```

### 9. 临时目录名只到日期,同日多次运行会混入旧数据

- **文件:** `src/backup_runner.rs:126`
- **影响:** `backup_{date}` 同一天第二次运行时,如果上一次因异常没清理干净,旧文件会残留进新备份。
- **建议:** 加时间戳或唯一后缀:`backup_{date}_{time}`。

### 10. git 仓库路径校验不足,且对非 bare 仓库不友好

- **文件:** `src/backup_entry.rs:137-143`
  ```rust
  cmd.arg("--git-dir").arg(repo_path)
  ```
- **影响:** `--git-dir` 要求指向 `.git` 目录(普通仓库)或 bare 仓库本身。如果用户通过文件选择器选了一个**普通工作目录**(而非 `.git` 子目录),`git archive` 会失败。原脚本是针对 bare repo 的特例,但工具化后用户可能选错。
- **建议:** 智能判断:若选中目录里有 `.git`,则用 `.git`;若是 bare(含 `HEAD`/`objects`),直接用;否则给明确报错。

---

## 🟡 轻微问题(代码质量)

### 11. `archive_name` 缺少路径穿越防护

- **文件:** `src/backup_entry.rs:30`
- **影响:** `temp_dir.join(archive_name)`,若用户(或配置文件)把 `archive_name` 设成 `"../../etc/evil"`,会写到临时目录外。虽是用户自配,但配置文件可被篡改。
- **建议:** sanitize:取最后一段、拒绝含分隔符的名字。

### 12. 输出文件名生成逻辑重复(DRY)

- **文件:** `src/backup_runner.rs:159-166`(实际生成)与 `:192-198`(预览)是两份几乎一样的代码。
- **建议:** 抽成一个 `fn build_output_filename(config, &date) -> String`,预览和实际共用。

### 13. 路径 UI 的同步顺序仍然脆弱

- **文件:** `src/ui/settings.rs:162, 180`
- **说明:** 注释说"先同步文本框,再处理浏览按钮"。实际上浏览按钮点击和文本编辑不会在同一帧同时发生,所以目前没 bug,但代码可读性差。
- **建议:** 改为:文本框 `.changed()` 时才回写,浏览按钮独立赋值。

### 14. 删除条目的索引管理在多选场景下有隐患

- **文件:** `src/ui/entry_list.rs:50, 142-144`
- **说明:** `remove_idx` 是单变量。当前每帧只能点一个按钮所以没问题,但若将来支持多选删除会出错。另外,删除中间条目后,正在编辑的更靠后条目的索引会前移(虽然代码把 `editing_index` 置 None 了,等于丢失编辑态)。可接受,但值得注意。

### 15. 日志系统双轨制

- **文件:** `src/app.rs:47` + `src/compressor.rs:175` 等
- **现象:** `app.rs:47` 把每条 UI 日志再用 `tracing::info!` 记一遍;而备份线程里的 `compressor.rs:175` 等又只用 tracing,不进 UI。结果:终端看到的是混合且重复的,UI 看到的是不完整的。
- **建议:** 二选一:要么备份逻辑也通过 sender 发日志,要么去掉 `app.rs:47` 的重复 tracing。

### 16. `BackupStatus` 比较的语义不精确

- **文件:** `src/app.rs:58-59`
- **现象:** `self.backup_status == BackupStatus::Done` 之后又用 `matches!(Error(_))`。
- **建议:** 统一成一个 `is_finished()` 方法,语义更清晰。

---

## 总结

| 级别 | 数量 | 关键项 |
|---|---|---|
| 🔴 严重 | 5 | Windows 检测失败、tar 级别失效、rar 非法级别、密码明文落盘、覆盖旧备份 |
| 🟠 中等 | 5 | zip/rar 无排除、缓存误判、panic 卡死、临时目录冲突、git 路径校验 |
| 🟡 轻微 | 6 | 路径穿越、DRY、UI 同步、删除索引、日志双轨、状态比较 |

**最值得先修的三件事:**
1. **#1(Windows 可用性)** — `which_command` 在 Windows 上不可用,直接导致工具在 Windows 无法运行
2. **#3(切换工具报错)** — 从高级别工具切到 rar 会生成非法 `-m9` 参数
3. **#4(密码明文)** — 安全隐患,真实密码以明文落盘到 `config.json`

以上三项直接影响"能否正常使用"和"安全性",建议优先处理。

---

## 附:建议的修复优先级清单

1. [#1] `which_command` 跨平台兼容(Windows 用 `where` 或 `which` crate)
2. [#4] 密码字段加 `#[serde(skip)]`,不落盘
3. [#3] 切换压缩工具时 clamp 压缩级别
4. [#2] tar 压缩级别处理(GNU tar `-I` 或 UI 禁用滑块)
5. [#8] 后台线程 panic 时检测 channel 关闭,避免 UI 卡死
6. [#5] 同日重复备份覆盖告警
7. [#6] 统一 zip/rar/tar 的排除项
8. 其余按优先级酌情处理
