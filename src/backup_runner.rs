use crate::backup_entry::{cleanup_temp_dir, collect_entry};
use crate::compressor::compress;
use crate::config::BackupConfig;
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;

/// 备份运行状态
#[derive(Debug, Clone, PartialEq)]
pub enum BackupStatus {
    Idle,
    Collecting,
    Compressing,
    Moving,
    Done,
    Error(String),
}

impl BackupStatus {
    /// 判断备份流程是否已结束（成功或失败）
    pub fn is_finished(&self) -> bool {
        matches!(self, BackupStatus::Done | BackupStatus::Error(_))
    }
}

/// 通过 channel 发送给 UI 的日志消息
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl LogMessage {
    pub fn info(msg: impl Into<String>) -> Self {
        LogMessage {
            level: LogLevel::Info,
            message: msg.into(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn ok(msg: impl Into<String>) -> Self {
        LogMessage {
            level: LogLevel::Success,
            message: msg.into(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn warn(msg: impl Into<String>) -> Self {
        LogMessage {
            level: LogLevel::Warn,
            message: msg.into(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        LogMessage {
            level: LogLevel::Error,
            message: msg.into(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        }
    }
}

/// 在后台线程中执行完整备份流程
///
/// 通过 sender 将日志消息发送到 UI 线程
/// 通过 status_sender 发送状态变更
pub fn run_backup(
    config: BackupConfig,
    sender: mpsc::Sender<LogMessage>,
    status_sender: mpsc::Sender<BackupStatus>,
) {
    let result = run_backup_inner(&config, &sender, &status_sender);

    // 无论成功失败都执行清理
    match result {
        Ok(output_path) => {
            let _ = status_sender.send(BackupStatus::Done);
            let _ = sender.send(LogMessage::info("-------------------------------------------"));
            let _ = sender.send(LogMessage::ok(format!("备份成功！最终位置: {}", output_path.display())));
        }
        Err(e) => {
            let _ = status_sender.send(BackupStatus::Error(e.clone()));
            let _ = sender.send(LogMessage::error(format!("备份失败: {e}")));
        }
    }
}

fn run_backup_inner(
    config: &BackupConfig,
    sender: &mpsc::Sender<LogMessage>,
    status_sender: &mpsc::Sender<BackupStatus>,
) -> Result<PathBuf, String> {
    // --- 阶段 0: 前置检查 ---
    let _ = sender.send(LogMessage::info("开始前置检查..."));

    if config.entries.is_empty() {
        return Err("没有配置任何备份条目，请先添加".to_string());
    }

    if !config.temp_dir.exists() {
        fs::create_dir_all(&config.temp_dir)
            .map_err(|e| format!("无法创建临时目录 {}: {e}", config.temp_dir.display()))?;
    }

    if !config.output_dir.exists() {
        fs::create_dir_all(&config.output_dir)
            .map_err(|e| format!("无法创建输出目录 {}: {e}", config.output_dir.display()))?;
    }

    let _ = sender.send(LogMessage::ok("前置检查通过"));

    // --- 阶段 1: 创建临时目录 ---
    let now = Local::now();
    let date = now.format("%Y%m%d").to_string();
    let time = now.format("%H%M%S").to_string();
    let temp_dir = config.temp_dir.join(format!("backup_{date}_{time}"));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("无法创建临时目录: {e}"))?;

    // 注册清理：函数退出时自动清理
    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            cleanup_temp_dir(&self.0);
        }
    }
    let _guard = TempGuard(temp_dir.clone());

    // --- 阶段 2: 收集文件 ---
    let _ = status_sender.send(BackupStatus::Collecting);
    let _ = sender.send(LogMessage::info("正在按条目分类整理文件..."));

    for (i, entry) in config.entries.iter().enumerate() {
        let _ = sender.send(LogMessage::info(
            format!("({}/{}) 处理: {}", i + 1, config.entries.len(), entry.display_label()),
        ));
        match collect_entry(entry, &temp_dir) {
            Ok(msg) => {
                let _ = sender.send(LogMessage::ok(msg));
            }
            Err(msg) => {
                let _ = sender.send(LogMessage::warn(msg));
            }
        }
    }

    // --- 阶段 3: 压缩 ---
    let _ = status_sender.send(BackupStatus::Compressing);
    let output_filename = build_output_filename(config, &date);
    let output_path = config.output_dir.join(&output_filename);

    // 压缩前诊断：检查临时目录内容
    let temp_entries: Vec<_> = fs::read_dir(&temp_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    if temp_entries.is_empty() {
        return Err("临时目录为空，没有收集到任何文件，请检查备份条目配置".to_string());
    }
    let _ = sender.send(LogMessage::info(format!(
        "临时目录包含 {} 个条目，路径: {}",
        temp_entries.len(),
        temp_dir.display()
    )));

    // 检测输出文件是否已存在，给出覆盖告警
    if output_path.exists() {
        let _ = sender.send(LogMessage::warn(format!(
            "输出文件已存在，将被覆盖: {}",
            output_path.display()
        )));
    }

    let level_str = config.compression_level.to_string();
    let _ = sender.send(LogMessage::info(format!(
        "正在压缩 (工具: {}, 级别: {})...",
        config.compressor.label(),
        level_str
    )));
    let _ = sender.send(LogMessage::info(format!(
        "输出路径: {}",
        output_path.display()
    )));

    compress(config, &temp_dir, &output_path)?;

    // --- 阶段 4: 确认输出 ---
    let _ = status_sender.send(BackupStatus::Moving);
    if output_path.exists() {
        let _ = sender.send(LogMessage::ok(format!(
            "压缩包已生成: {}",
            output_path.display()
        )));
    } else {
        return Err("压缩包未生成，检查压缩工具是否正常".to_string());
    }

    Ok(output_path)
}

/// 构建输出文件名（供实际备份和预览共用）
pub fn build_output_filename(config: &BackupConfig, date: &str) -> String {
    let name = config.output_filename_pattern.replace("{date}", date);
    format!("{}.{}", name, config.compressor.extension())
}

/// 生成输出文件名预览
pub fn preview_output_filename(config: &BackupConfig) -> String {
    let date = Local::now().format("%Y%m%d").to_string();
    build_output_filename(config, &date)
}
