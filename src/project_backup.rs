use crate::builder::ArtifactBuilder;
use crate::compressor::compress;
use crate::config::BackupConfig;
use crate::validation::FileValidator;
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::backup_runner::LogMessage;

/// 项目备份配置
#[derive(Debug, Clone)]
pub struct ProjectBackup {
    /// 要备份的项目路径
    pub source_path: PathBuf,
    /// 备份目标路径
    pub target_path: PathBuf,
    /// 压缩包名称（不含扩展名）
    pub archive_name: String,
}

impl Default for ProjectBackup {
    fn default() -> Self {
        ProjectBackup {
            source_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            target_path: crate::config::default_output_dir(),
            archive_name: "project-backup".to_string(),
        }
    }
}

impl ProjectBackup {
    /// 验证配置是否有效
    pub fn validate(&self) -> Result<(), String> {
        if !self.source_path.exists() {
            return Err(format!("源路径不存在: {}", self.source_path.display()));
        }

        if !self.target_path.exists() {
            return Err(format!("目标路径不存在: {}", self.target_path.display()));
        }

        if self.archive_name.trim().is_empty() {
            return Err("压缩包名称不能为空".to_string());
        }

        Ok(())
    }

    /// 执行完整的验证和构建流程，生成备份摘要
    /// 在启动备份线程前调用，确认通过后再执行 `run_backup`
    pub fn validate_project(
        &self,
        sender: &mpsc::Sender<LogMessage>,
        compression_level: u32,
        exclude_patterns: &[String],
    ) -> Result<String, String> {
        let _ = sender.send(LogMessage::info("开始验证项目..."));
        let os = std::env::consts::OS;
        let include_skills =
            std::env::var("INCLUDE_FILE_SKILLS").unwrap_or_default() == "1";

        // 1. 基础配置验证
        self.validate()?;

        // 2. 检查运行必需文件
        let validator = FileValidator::new(include_skills);
        let _ = sender.send(LogMessage::info("正在检查必需文件..."));
        validator.validate_essential(&self.source_path).map_err(|e| {
            format!("必需文件检查失败: {}", e)
        })?;
        let _ = sender.send(LogMessage::ok("必需文件检查通过"));

        // 3. 验证平台二进制文件
        let _ = sender.send(LogMessage::info("正在验证平台二进制文件..."));
        validator.validate_binaries(&self.source_path).map_err(|e| {
            format!("二进制文件验证失败: {}", e)
        })?;
        let _ = sender.send(LogMessage::ok("二进制文件验证通过"));

        // 4. 自动构建缺失/过期的产物
        let _ = sender.send(LogMessage::info("正在构建缺失产物..."));
        if let Err(e) = ArtifactBuilder::build_missing(&self.source_path) {
            let _ = sender.send(LogMessage::warn(format!("构建失败: {}", e)));
            // 构建失败不阻断流程，因为某些项目可能不需要构建
        } else {
            let _ = sender.send(LogMessage::ok("构建完成"));
        }

        // 5. 生成备份摘要
        let summary = self.build_summary(os, include_skills, compression_level, exclude_patterns);
        let _ = sender.send(LogMessage::info("备份摘要已生成"));

        Ok(summary)
    }

    /// 生成备份内容摘要
    fn build_summary(
        &self,
        os: &str,
        include_skills: bool,
        compression_level: u32,
        exclude_patterns: &[String],
    ) -> String {
        let mut s = String::new();
        s.push_str(&format!("📋 备份摘要\n"));
        s.push_str(&format!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"));
        s.push_str(&format!("项目路径: {}\n", self.source_path.display()));
        s.push_str(&format!("目标路径: {}\n", self.target_path.display()));
        s.push_str(&format!("压缩包名称: {}.7z\n", self.archive_name));
        s.push_str(&format!("平台: {}\n", os));
        s.push_str("\n");

        s.push_str("【备份内容】\n");
        s.push_str(&format!("  项目目录: {}\n", self.source_path.display()));
        s.push_str("  备份方式: 完整目录打包\n");
        s.push_str("  包含: 全部项目文件 (递归)\n");

        s.push_str("\n【排除规则】\n");
        let excludes = [
            ".DS_Store, Thumbs.db, .qoder",
            ".git/, node_modules/, __pycache__/, .cache/",
            "__tests__/, .vscode/, .claude/, .zed/",
            ".arts/, .codex/, copilot/",
            "*.log, *.pid, *.pyc",
            "隐藏文件/目录 (以 . 开头，但 .env 保留)",
        ];
        for e in &excludes {
            s.push_str(&format!("  ✗ {}\n", e));
        }

        // 用户自定义排除规则
        for p in exclude_patterns {
            s.push_str(&format!("  ✗ {} (用户自定义)\n", p));
        }

        if include_skills {
            s.push_str("\n【技能模块】\n");
            s.push_str("  ✅ modules/skills/ 已包含\n");
        }

        s.push_str("\n【验证结果】\n");
        s.push_str(&format!("  平台: {}\n", os));
        let bin_paths = match os {
            "macos" => vec![
                "dist/darwin-{arm64,amd64}/Ashen-Protocol",
                "dist/darwin-{arm64,amd64}/ashen-tools",
            ],
            "windows" => vec![
                "dist/windows-amd64/Ashen-Protocol.exe",
                "dist/windows-amd64/ashen-tools.exe",
            ],
            _ => vec![],
        };
        s.push_str("  二进制验证: ✅ 已通过\n");
        for b in &bin_paths {
            s.push_str(&format!("    {}\n", b));
        }

        s.push_str("\n【加密设置】\n");
        s.push_str(&format!("  • 压缩级别: {}\n", compression_level));
        s.push_str("  • 文件名加密: 是\n");
        s.push_str("  • 完整性验证: SHA256\n");

        s.push_str("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
        s.push_str("请确认以上备份内容");
        s
    }

    /// 执行项目备份（复制 + 压缩）
    pub fn run_backup(
        &self,
        config: &BackupConfig,
        sender: &mpsc::Sender<LogMessage>,
    ) -> Result<PathBuf, String> {
        let _ = sender.send(LogMessage::info("开始执行备份..."));

        // 验证配置
        self.validate()?;

        // 创建临时目录
        let now = Local::now();
        let date = now.format("%Y%m%d").to_string();
        let time = now.format("%H%M%S").to_string();
        let temp_dir = config.temp_dir.join(format!("project_backup_{date}_{time}"));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("无法创建临时目录: {e}"))?;

        // 注册清理
        struct TempGuard(PathBuf);
        impl Drop for TempGuard {
            fn drop(&mut self) {
                if self.0.exists() {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
        }
        let _guard = TempGuard(temp_dir.clone());

        let _ = sender.send(LogMessage::info("正在准备打包树..."));

        // 准备打包树 - 复制源目录到临时目录
        let package_name = sanitize_package_name(&self.archive_name);
        let package_dir = temp_dir.join(&package_name);
        fs::create_dir_all(&package_dir)
            .map_err(|e| format!("无法创建打包目录: {e}"))?;

        // 复制源目录内容到打包目录（应用排除规则）
        copy_project_tree(&self.source_path, &package_dir, &config.project_exclude_patterns, sender)?;

        let _ = sender.send(LogMessage::info("正在执行压缩..."));

        // 构建输出文件名（含日期时间戳）
        let output_filename = format!("{}_{}_{}.{}", package_name, date, time, config.compressor.extension());
        let output_path = self.target_path.join(&output_filename);

        // 检查输出文件是否已存在
        if output_path.exists() {
            let _ = sender.send(LogMessage::warn(format!(
                "输出文件已存在，将被覆盖: {}",
                output_path.display()
            )));
        }

        let _ = sender.send(LogMessage::info(format!(
            "压缩参数: 工具={}, 级别={}",
            config.compressor.label(),
            config.compression_level
        )));

        // 执行压缩
        compress(config, &temp_dir, &output_path)?;

        // 验证输出文件
        if !output_path.exists() {
            return Err("压缩失败：输出文件未生成".to_string());
        }

        let _ = sender.send(LogMessage::ok(format!(
            "项目备份成功: {}",
            output_path.display()
        )));

        Ok(output_path)
    }
}

/// 清理包名称，防止路径穿越
fn sanitize_package_name(name: &str) -> String {
    let safe = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project-backup".to_string());

    if safe.is_empty() || safe == "." || safe == ".." {
        "project-backup".to_string()
    } else {
        safe
    }
}

/// 复制项目目录树到打包目录
fn copy_project_tree(
    src: &Path,
    dest: &Path,
    exclude_patterns: &[String],
    sender: &mpsc::Sender<LogMessage>,
) -> Result<(), String> {
    if !src.exists() {
        return Err(format!("源路径不存在: {}", src.display()));
    }

    let src_path_str = src.display().to_string();
    let _ = sender.send(LogMessage::info(format!(
        "正在复制: {} -> {}",
        src.display(),
        dest.display()
    )));

    if !exclude_patterns.is_empty() {
        let _ = sender.send(LogMessage::info(format!(
            "用户排除规则: {:?}",
            exclude_patterns
        )));
    }

    // 使用 walkdir 遍历并复制文件
    for entry in walkdir::WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let relative = e.path().strip_prefix(src).ok();
            !should_exclude_entry(e, relative, exclude_patterns)
        })
        .filter_map(|e| e.ok())
    {
        let src_path = entry.path();
        let relative_path = src_path
            .strip_prefix(src)
            .map_err(|e| format!("路径处理失败: {e}"))?;
        let dest_path = dest.join(relative_path);

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)
                .map_err(|e| format!("创建目录失败 {}: {e}", dest_path.display()))?;
        } else if src_path.is_file() {
            // 确保父目录存在
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("创建父目录失败: {e}"))?;
            }
            fs::copy(src_path, &dest_path)
                .map_err(|e| format!("复制文件失败 {}: {e}", src_path.display()))?;
        }
    }

    let _ = sender.send(LogMessage::ok(format!(
        "复制完成: {}",
        src_path_str
    )));

    Ok(())
}

/// 判断是否应该排除某个条目
fn should_exclude_entry(
    entry: &walkdir::DirEntry,
    relative: Option<&Path>,
    exclude_patterns: &[String],
) -> bool {
    let name = entry.file_name().to_string_lossy();
    let name_str = name.as_ref();

    // 特定文件排除
    match name_str {
        ".DS_Store" | "Thumbs.db" | ".qoder" => return true,
        _ => {}
    }

    // 目录排除
    if entry.file_type().is_dir() {
        match name_str {
            ".git" | "node_modules" | "__pycache__" | ".cache" | "__tests__"
            | ".vscode" | ".claude" | ".zed" | ".arts" | ".codex"
            | "copilot" => return true,
            _ => {}
        }
    }
    // 文件按扩展名排除
    else if let Some(ext) = entry.path().extension() {
        match ext.to_string_lossy().as_ref() {
            "log" | "pid" | "pyc" => return true,
            _ => {}
        }
    }

    // 用户自定义排除模式（精确匹配路径组件的首、中、尾）
    if !exclude_patterns.is_empty() {
        if let Some(rel) = relative {
            let rel_str = rel.to_string_lossy();
            for pattern in exclude_patterns {
                if rel_str == pattern.as_str()
                    || rel_str.starts_with(&format!("{}/", pattern))
                    || rel_str.contains(&format!("/{}/", pattern))
                    || rel_str.ends_with(&format!("/{}", pattern))
                {
                    return true;
                }
            }
        }
    }

    // 隐藏文件/目录白名单（必需保留的）
    const PRESERVED_DOT: &[&str] = &[".env"];
    if PRESERVED_DOT.contains(&name_str) {
        return false;
    }

    // 排除隐藏文件和目录
    name_str.starts_with('.')
}
