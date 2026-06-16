# Backup Utility

跨平台（macOS / Windows）的 GUI 备份工具。用户可自定义任意数量的备份条目（本地文件/目录、Git 仓库分支），选择压缩工具与级别，按需加密，最终生成一个打包好的归档文件。

由原始的 `auto_backup.sh`（加密 7z 备份脚本）用 Rust + egui 重构而来。

## 功能特性

- **自定义备份条目（无上限）** — 添加任意数量的本地文件或目录，并可指定其在压缩包内的名称。
- **支持四种压缩工具** — 7z / zip / rar / tar.gz，可自由选择并自定义压缩级别。
- **自动检测压缩工具** — 自动从 PATH 和 Homebrew 路径查找压缩工具，UI 实时显示可用性状态。
- **可选密码加密** — 支持设置密码；对 7z / rar 还支持文件名加密（不输密码连包内文件名都无法查看）。
- **Git 仓库备份** — 支持从 Git 仓库导出指定分支（`git archive`），分支名可在编辑模式下修改。
- **智能过滤** — 自动跳过 `.DS_Store`、`Thumbs.db`、`node_modules`、`.git`、`.log` 等无关文件。
- **配置持久化** — 条目与设置自动保存为 JSON，下次打开自动恢复。
- **跨平台** — 兼容 macOS 与 Windows。

## 环境要求

### 构建

- **Rust 工具链**（推荐通过 [rustup](https://rustup.rs/) 安装），Edition 2024
- **Cargo**（随 Rust 一同安装）

### 外部依赖（至少安装一种压缩工具）

| 压缩工具 | macOS 安装 | Windows 安装 |
|---|---|---|
| 7z | `brew install p7zip` | 安装 [7-Zip](https://www.7-zip.org/) |
| zip | 系统自带 | 通常已内置 |
| rar | 从 [rarlab](https://www.rarlab.com/) 下载 | 从 [rarlab](https://www.rarlab.com/) 下载 |
| tar | 系统自带 | Windows 自带 bsdtar |
| git | `brew install git` | [git-scm.com](https://git-scm.com/) |

> 若工具未加入系统 PATH，可在 UI 的「路径」输入框中手动指定可执行文件位置。macOS 上 Homebrew 安装的工具（`/opt/homebrew/bin`）会被自动检测。

## 快速开始

```bash
git clone <repo-url> Backup_Utility
cd Backup_Utility
cargo build --release
```

构建产物：

```bash
# macOS
./target/release/backup_utility

# Windows
.\target\release\backup_utility.exe
```

## 使用方法

主界面自上而下分为五个区域：

### 备份条目列表

点击右上角按钮添加条目：

| 按钮 | 作用 |
|---|---|
| **+ 添加目录** | 弹出目录选择对话框，选中后自动以目录名作为包内名称 |
| **+ 添加文件** | 弹出文件选择对话框，选中后自动以文件名作为包内名称 |
| **+ 添加 Git 仓库** | 弹出目录选择对话框，选中 Git 仓库后默认导出 main 分支 |
| **删除选中** | 删除当前正在编辑的条目 |

每个条目一行，包含：类型、源路径、分支（仅 Git 条目）、包内名称、操作按钮。

> 点击「编辑」可修改源路径、分支名和包内名称。编辑「包内名称」可自定义条目在压缩包内的组织结构。

### 压缩设置

| 选项 | 说明 |
|---|---|
| **工具** | 7z / zip / rar / tar.gz 四选一，右侧实时显示可用性状态 |
| **路径** | 压缩工具可执行文件路径，留空则自动查找 |
| **级别** | 拖动滑块设置压缩级别（7z/zip/tar: 0-9，rar: 0-5） |
| **密码** | 留空则不加密 |
| **加密文件名** | 勾选后（仅 7z/rar 支持），不输密码无法查看包内文件名 |
| **文件名** | 输出文件名模板，`{date}` 会替换为当日日期 |

### 路径设置

- **临时目录** — 备份过程中临时文件存放位置（建议使用快速磁盘，如 macOS 的 RamDisk）
- **输出目录** — 最终压缩包的存放位置

### 操作按钮

- **开始备份** — 启动备份（运行期间此按钮禁用）
- **保存配置** — 将当前所有设置写入配置文件
- **清空日志** — 清空日志面板

### 备份流程

点击「开始备份」后，程序依次执行：

1. **前置检查** — 校验条目非空、目录可创建、压缩工具可用
2. **收集** — 将各条目复制或 `git archive` 到临时目录
3. **压缩** — 按所选工具与级别打包
4. **完成** — 输出压缩包到目标目录，自动清理临时目录

## 配置文件

配置自动保存到以下位置：

| 系统 | 路径 |
|---|---|
| macOS | `~/Library/Application Support/backup_utility/config.json` |
| Windows | `%LOCALAPPDATA%\backup_utility\config.json` |

示例：

```json
{
  "entries": [
    { "LocalFile": { "source_path": "/Users/me/Notes", "archive_name": "notes" } },
    { "GitArchive": { "repo_path": "/path/to/repo.git", "branch": "main", "archive_name": "myrepo" } }
  ],
  "temp_dir": "/Volumes/RamDisk",
  "output_dir": "/Users/me/backup",
  "compressor": "SevenZip",
  "compressor_path": null,
  "compression_level": 9,
  "encrypt_filenames": true,
  "output_filename_pattern": "backup{date}"
}
```

> 密码字段不会被写入配置文件（`#[serde(skip)]`），仅在内存中保留。每次运行需在 UI 中重新输入。

## 项目结构

```
Backup_Utility/
├── Cargo.toml              # 依赖与包元数据
├── LICENSE                 # MIT 许可证
├── assets/
│   └── NotoSansSC-Subset.ttf  # 中文字体子集（~200KB）
├── auto_backup.sh          # 原始 bash 脚本（保留参考）
├── docs/                   # 文档与代码审查报告
└── src/
    ├── main.rs             # 入口、字体配置、全局主题
    ├── app.rs              # 主应用状态 + UI 布局 + 后台线程管理
    ├── config.rs           # 数据模型 + JSON 持久化
    ├── backup_entry.rs     # 本地文件复制 + git archive 导出
    ├── backup_runner.rs    # 备份流程编排
    ├── compressor.rs       # 四种压缩引擎抽象 + 工具路径检测
    └── ui/
        ├── mod.rs
        ├── entry_list.rs   # 备份条目列表 UI
        ├── settings.rs     # 压缩设置 + 路径设置面板
        └── progress.rs     # 日志与进度面板
```

## 技术栈

| 用途 | 库 |
|---|---|
| GUI 框架 | [eframe](https://github.com/emilk/egui) (egui) |
| 文件选择对话框 | [rfd](https://github.com/PolyMeilex/rfd) |
| 序列化 | [serde](https://serde.rs/) + serde_json |
| 日期处理 | [chrono](https://github.com/chronotope/chrono) |
| 跨平台路径 | [dirs](https://github.com/soc/dirs-rs) |
| 日志 | [tracing](https://github.com/tokio-rs/tracing) |

后台备份任务通过 `std::thread` + `std::sync::mpsc` 与 UI 线程通信，不阻塞界面。

## 许可证

[MIT License](./LICENSE)
