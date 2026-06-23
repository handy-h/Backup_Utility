use crate::config::{BackupConfig, CompressorType};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 统一的排除项列表（所有压缩格式共用）
const EXCLUDE_PATTERNS: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    ".qoder",
    ".vscode",
    "node_modules",
];

/// macOS 上常见的包管理器安装路径（GUI 应用的 PATH 通常不包含这些）
#[cfg(target_os = "macos")]
const EXTRA_BIN_PATHS: &[&str] = &[
    "/opt/homebrew/bin",  // Apple Silicon Homebrew
    "/usr/local/bin",     // Intel Homebrew / 手动安装
];

/// 构建并执行压缩命令
pub fn compress(
    config: &BackupConfig,
    temp_dir: &Path,
    output_file: &Path,
) -> Result<(), String> {
    let compressor = &config.compressor;

    // 获取可执行文件路径
    let exe = resolve_compressor_exe(compressor, &config.compressor_path)?;
    tracing::info!("压缩工具路径: {}", exe.display());

    match compressor {
        CompressorType::SevenZip => compress_7z(&exe, temp_dir, output_file, config)?,
        CompressorType::Zip => compress_zip(&exe, temp_dir, output_file, config)?,
        CompressorType::Rar => compress_rar(&exe, temp_dir, output_file, config)?,
        CompressorType::Tar => compress_tar(&exe, temp_dir, output_file, config)?,
    };

    // 命令返回成功但文件不存在，给出更具体的诊断
    if !output_file.exists() {
        return Err(format!(
            "{} 命令执行完毕但输出文件未生成 (目标路径: {})",
            compressor.label(),
            output_file.display()
        ));
    }

    Ok(())
}

/// 启动时检测压缩工具是否可用，返回可用则 Ok(实际路径)，不可用则 Err(提示信息)
pub fn check_compressor_available(
    compressor: &CompressorType,
    custom_path: &Option<PathBuf>,
) -> Result<PathBuf, String> {
    resolve_compressor_exe(compressor, custom_path)
}

/// 解析压缩工具的可执行文件路径
fn resolve_compressor_exe(
    compressor: &CompressorType,
    custom_path: &Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(path) = custom_path {
        if path.exists() {
            return Ok(path.clone());
        }
        return Err(format!(
            "指定的压缩工具不存在: {}",
            path.display()
        ));
    }

    let cmd_name = compressor.default_command();

    // 1. 先尝试在 PATH 中查找
    if which_command(cmd_name) {
        return Ok(PathBuf::from(cmd_name));
    }

    // 2. macOS GUI 应用 PATH 有限，额外检查 Homebrew 等常见路径
    #[cfg(target_os = "macos")]
    {
        for dir in EXTRA_BIN_PATHS {
            let candidate = PathBuf::from(dir).join(cmd_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "未找到 {cmd_name} 命令，请安装或在「压缩设置」中手动指定路径"
    ))
}

/// 跨平台检查命令是否在 PATH 中
fn which_command(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    let program = "where";
    #[cfg(not(target_os = "windows"))]
    let program = "which";

    Command::new(program)
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 7z 压缩
fn compress_7z(
    exe: &Path,
    source: &Path,
    output: &Path,
    config: &BackupConfig,
) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.arg("a")
        .arg(output)
        .arg(source)
        .arg(format!("-mx={}", config.compression_level))
        .arg("-ms=on"); // 固实压缩

    // 密码
    if let Some(pw) = &config.password {
        cmd.arg(format!("-p{pw}"));
    }

    // 文件名加密
    if config.password.is_some() && config.encrypt_filenames {
        cmd.arg("-mhe=on");
    } else if config.password.is_some() {
        cmd.arg("-mhe=off");
    }

    // 完整性验证 (SHA256)
    if config.password.is_some() {
        cmd.arg("-scrcSHA256");
    }

    // 排除项
    cmd.arg("-xr!.DS_Store")
        .arg("-xr!Thumbs.db")
        .arg("-xr!.qoder")
        .arg("-xr!.vscode")
        .arg("-xr!node_modules");

    execute_cmd(&mut cmd, "7z")
}

/// zip 压缩
fn compress_zip(
    exe: &Path,
    source: &Path,
    output: &Path,
    config: &BackupConfig,
) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.arg("-r")
        .arg(format!("-{}", config.compression_level))
        .arg(output);

    // 密码
    if let Some(pw) = &config.password {
        cmd.arg(format!("-P{pw}"));
    }

    cmd.arg(source);

    // 排除项
    for pattern in EXCLUDE_PATTERNS {
        cmd.arg("-x").arg(format!("*{pattern}"));
    }

    execute_cmd(&mut cmd, "zip")
}

/// rar 压缩
fn compress_rar(
    exe: &Path,
    source: &Path,
    output: &Path,
    config: &BackupConfig,
) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.arg("a")
        .arg(output)
        .arg(format!("-m{}", config.compression_level));

    // 密码
    if let Some(pw) = &config.password {
        if config.encrypt_filenames {
            cmd.arg(format!("-hp{pw}"));
        } else {
            cmd.arg(format!("-p{pw}"));
        }
    }

    cmd.arg(source);

    // 排除项
    for pattern in EXCLUDE_PATTERNS {
        cmd.arg(format!("-x*{pattern}"));
    }

    execute_cmd(&mut cmd, "rar")
}

/// tar.gz 压缩（使用管道方式确保压缩级别跨平台生效）
fn compress_tar(
    exe: &Path,
    source: &Path,
    output: &Path,
    config: &BackupConfig,
) -> Result<(), String> {
    if config.password.is_some() {
        return Err("tar 格式不支持密码加密，请选择 7z、zip 或 rar".to_string());
    }

    let gzip_level = config.compression_level.min(9);
    tracing::info!("正在执行 tar | gzip 管道命令...");

    // 启动 gzip 子进程，读取 stdin、写入输出文件
    let output_file = std::fs::File::create(output)
        .map_err(|e| format!("创建输出文件失败: {e}"))?;

    let mut gzip_child = Command::new("gzip")
        .arg(format!("-{gzip_level}"))
        .stdin(std::process::Stdio::piped())
        .stdout(output_file)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 gzip 失败: {e}"))?;

    // 构建 tar 命令
    let mut tar_cmd = Command::new(exe);
    tar_cmd.arg("cf")
        .arg("-"); // 输出到 stdout

    // 排除项
    for pattern in EXCLUDE_PATTERNS {
        tar_cmd.arg("--exclude").arg(format!("*/{pattern}"));
    }

    tar_cmd.arg("-C").arg(source).arg(".");

    // tar 输出管道到 gzip 的 stdin
    let tar_output = tar_cmd
        .stdout(gzip_child.stdin.take().unwrap())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("执行 tar 失败: {e}"))?;

    // 等待 gzip 完成
    let gzip_output = gzip_child
        .wait_with_output()
        .map_err(|e| format!("等待 gzip 失败: {e}"))?;

    if !tar_output.status.success() {
        let stderr = String::from_utf8_lossy(&tar_output.stderr);
        return Err(format!("tar 压缩失败: {stderr}"));
    }

    if !gzip_output.status.success() {
        let stderr = String::from_utf8_lossy(&gzip_output.stderr);
        return Err(format!("gzip 压缩失败: {stderr}"));
    }

    tracing::info!("✅ tar.gz 压缩完成");
    Ok(())
}

/// 执行命令并检查结果
fn execute_cmd(cmd: &mut Command, name: &str) -> Result<(), String> {
    tracing::info!("正在执行 {name} 命令...");
    let output = cmd
        .output()
        .map_err(|e| format!("执行 {name} 命令失败: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        for line in stdout.lines() {
            if !line.trim().is_empty() {
                tracing::info!("[{name}] {line}");
            }
        }
        tracing::info!("[{name}] 压缩完成");
        Ok(())
    } else {
        // 7z 等工具的错误信息可能输出到 stdout 或 stderr，都包含在错误中
        let mut detail = String::new();
        if !stderr.trim().is_empty() {
            detail.push_str(&stderr);
        }
        if !stdout.trim().is_empty() {
            if !detail.is_empty() {
                detail.push_str("\n");
            }
            detail.push_str(&stdout);
        }
        if detail.is_empty() {
            detail = format!("退出码: {}", output.status.code().unwrap_or(-1));
        }
        Err(format!("{name} 压缩失败:\n{detail}"))
    }
}
