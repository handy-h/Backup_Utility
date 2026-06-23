use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 备份条目类型：支持本地文件/目录和 Git 仓库
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupEntry {
    /// 本地文件或目录
    LocalFile {
        source_path: PathBuf,
        archive_name: String,
    },
    /// 从 Git 仓库导出指定分支
    GitArchive {
        repo_path: PathBuf,
        branch: String,
        archive_name: String,
    },
}

impl BackupEntry {
    /// 返回条目在压缩包中的名称
    #[allow(dead_code)]
    pub fn archive_name(&self) -> &str {
        match self {
            BackupEntry::LocalFile { archive_name, .. } => archive_name,
            BackupEntry::GitArchive { archive_name, .. } => archive_name,
        }
    }

    /// 返回条目的显示标签
    pub fn display_label(&self) -> String {
        match self {
            BackupEntry::LocalFile { source_path, .. } => {
                format!("[文件] {}", source_path.display())
            }
            BackupEntry::GitArchive {
                repo_path, branch, ..
            } => {
                format!("[Git] {} (branch: {})", repo_path.display(), branch)
            }
        }
    }

    /// 返回源路径（用于排序/去重参考）
    #[allow(dead_code)]
    pub fn source_path(&self) -> &Path {
        match self {
            BackupEntry::LocalFile { source_path, .. } => source_path,
            BackupEntry::GitArchive { repo_path, .. } => repo_path,
        }
    }
}

/// 支持的压缩工具类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CompressorType {
    #[default]
    SevenZip,
    Zip,
    Rar,
    Tar,
}

impl CompressorType {
    pub fn label(&self) -> &'static str {
        match self {
            CompressorType::SevenZip => "7z",
            CompressorType::Zip => "zip",
            CompressorType::Rar => "rar",
            CompressorType::Tar => "tar.gz",
        }
    }

    pub fn all() -> &'static [CompressorType] {
        &[
            CompressorType::SevenZip,
            CompressorType::Zip,
            CompressorType::Rar,
            CompressorType::Tar,
        ]
    }

    /// 是否支持密码加密
    pub fn supports_password(&self) -> bool {
        matches!(self, CompressorType::SevenZip | CompressorType::Zip | CompressorType::Rar)
    }

    /// 是否支持文件名加密
    pub fn supports_encrypt_filenames(&self) -> bool {
        matches!(self, CompressorType::SevenZip | CompressorType::Rar)
    }

    /// 返回命令名称（用于查找可执行文件）
    pub fn default_command(&self) -> &'static str {
        match self {
            CompressorType::SevenZip => "7z",
            CompressorType::Zip => "zip",
            CompressorType::Rar => "rar",
            CompressorType::Tar => "tar",
        }
    }

    /// 默认压缩级别
    #[allow(dead_code)]
    pub fn default_level(&self) -> u32 {
        match self {
            CompressorType::SevenZip => 9,
            CompressorType::Zip => 9,
            CompressorType::Rar => 5,
            CompressorType::Tar => 9,
        }
    }

    /// 最大压缩级别
    pub fn max_level(&self) -> u32 {
        match self {
            CompressorType::SevenZip => 9,
            CompressorType::Zip => 9,
            CompressorType::Rar => 5,
            CompressorType::Tar => 9,
        }
    }

    /// 输出文件扩展名
    pub fn extension(&self) -> &'static str {
        match self {
            CompressorType::SevenZip => "7z",
            CompressorType::Zip => "zip",
            CompressorType::Rar => "rar",
            CompressorType::Tar => "tar.gz",
        }
    }
}

/// 全局备份配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// 备份条目列表
    pub entries: Vec<BackupEntry>,
    /// 临时文件存放位置
    pub temp_dir: PathBuf,
    /// 最终压缩包存放位置
    pub output_dir: PathBuf,
    /// 压缩工具类型
    pub compressor: CompressorType,
    /// 压缩工具可执行文件路径（用户可自定义，为空则用默认命令名）
    pub compressor_path: Option<PathBuf>,
    /// 压缩级别
    pub compression_level: u32,
    /// 密码（None 表示不加密）
    #[serde(skip)]
    pub password: Option<String>,
    /// 加密时是否隐藏文件名
    pub encrypt_filenames: bool,
    /// 输出文件名模板，{date} 会被替换为日期
    pub output_filename_pattern: String,
    /// 项目备份配置 - 源路径
    pub project_backup_source: PathBuf,
    /// 项目备份配置 - 目标路径
    pub project_backup_target: PathBuf,
    /// 项目备份配置 - 压缩包名称
    pub project_backup_archive_name: String,
    /// 项目备份 - 用户自定义排除模式（目录/文件名，匹配任意路径组件）
    pub project_exclude_patterns: Vec<String>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        BackupConfig {
            entries: Vec::new(),
            temp_dir: default_temp_dir(),
            output_dir: default_output_dir(),
            compressor: CompressorType::SevenZip,
            compressor_path: None,
            compression_level: 9,
            password: None,
            encrypt_filenames: true,
            output_filename_pattern: "backup{date}".to_string(),
            project_backup_source: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            project_backup_target: default_output_dir(),
            project_backup_archive_name: "project-backup".to_string(),
            project_exclude_patterns: Vec::new(),
        }
    }
}

/// 获取默认临时目录
fn default_temp_dir() -> PathBuf {
    // 优先尝试 RAM Disk（macOS）
    let ram_disk = PathBuf::from("/Volumes/RamDisk");
    if ram_disk.is_dir() {
        return ram_disk;
    }
    // 回退到系统临时目录
    std::env::temp_dir()
}

/// 获取默认输出目录
pub fn default_output_dir() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("backup")
}

/// 配置文件路径
pub fn config_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        })
        .join("backup_utility");

    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("config.json")
}

/// 加载配置
pub fn load_config() -> BackupConfig {
    let path = config_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => return config,
                Err(e) => {
                    tracing::warn!("配置文件解析失败: {e}，使用默认配置");
                }
            },
            Err(e) => {
                tracing::warn!("配置文件读取失败: {e}，使用默认配置");
            }
        }
    }
    BackupConfig::default()
}

/// 保存配置
pub fn save_config(config: &BackupConfig) -> Result<(), String> {
    let path = config_path();
    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&path, content).map_err(|e| format!("写入失败: {e}"))?;
    tracing::info!("配置已保存到: {}", path.display());
    Ok(())
}
