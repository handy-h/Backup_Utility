use crate::config::BackupEntry;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// 收集备份条目到指定的临时目录
pub fn collect_entry(
    entry: &BackupEntry,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    match entry {
        BackupEntry::LocalFile {
            source_path,
            archive_name,
        } => collect_local(source_path, archive_name, temp_dir),
        BackupEntry::GitArchive {
            repo_path,
            branch,
            archive_name,
        } => collect_git(repo_path, branch, archive_name, temp_dir),
    }
}

/// 清理 archive_name，防止路径穿越
fn sanitize_archive_name(name: &str) -> String {
    // 取最后一段路径组件，拒绝空名称
    let safe = std::path::Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    if safe.is_empty() || safe == "." || safe == ".." {
        "unnamed".to_string()
    } else {
        safe
    }
}

/// 收集本地文件/目录到临时目录
fn collect_local(
    source_path: &std::path::Path,
    archive_name: &str,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    let safe_name = sanitize_archive_name(archive_name);
    let dest = temp_dir.join(&safe_name);

    if !source_path.exists() {
        return Err(format!("源路径不存在: {}", source_path.display()));
    }

    if source_path.is_dir() {
        fs::create_dir_all(&dest)
            .map_err(|e| format!("创建目标目录失败: {e}"))?;

        // 使用递归复制，排除隐藏文件和缓存
        copy_dir_recursive(source_path, &dest)?;
        Ok(format!("已复制目录: {archive_name}"))
    } else if source_path.is_file() {
        // 确保父目录存在
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建父目录失败: {e}"))?;
        }
        fs::copy(source_path, &dest)
            .map_err(|e| format!("复制文件失败: {e}"))?;
        Ok(format!("已复制文件: {archive_name}"))
    } else {
        Err(format!("不支持的源类型: {}", source_path.display()))
    }
}

/// 递归复制目录，过滤不需要的文件
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("读取目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // 跳过隐藏文件和缓存目录
        if should_skip(&file_name_str) {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&file_name);

        if src_path.is_dir() {
            // 跳过 Obsidian 的 copilot 缓存目录
            if is_cache_dir(&src_path) {
                continue;
            }
            fs::create_dir_all(&dest_path)
                .map_err(|e| format!("创建子目录失败: {e}"))?;
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            // 跳过 .log 文件
            if is_log_file(&src_path) {
                continue;
            }
            fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("复制文件失败: {} — {e}", src_path.display()))?;
        }
    }
    Ok(())
}

/// 判断文件/目录是否应该跳过
fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".DS_Store"
            | "Thumbs.db"
            | ".qoder"
            | ".vscode"
            | "node_modules"
            | ".git"
            | "__pycache__"
    )
}

/// 判断是否是缓存目录（按路径分量匹配，避免误判）
fn is_cache_dir(path: &std::path::Path) -> bool {
    // 精确匹配目录名 ".cache"
    let is_dot_cache = path
        .file_name()
        .map(|n| n == ".cache")
        .unwrap_or(false);

    // 按路径分量匹配 copilot 缓存子目录
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    is_dot_cache
        || components.windows(2).any(|w| {
            w == ["copilot", "history"] || w == ["copilot", "cache"]
        })
}

/// 判断是否是日志文件
fn is_log_file(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
}

/// 解析有效的 git 目录路径
///
/// 智能判断：若是 bare 仓库则直接使用；若含 .git 子目录则用 .git；否则报错
fn resolve_git_dir(repo_path: &std::path::Path) -> Result<PathBuf, String> {
    // 若路径本身就是有效的 git 目录（bare 仓库或 .git 目录）
    if repo_path.join("HEAD").exists() && repo_path.join("objects").exists() {
        return Ok(repo_path.to_path_buf());
    }
    // 若选中目录里有 .git 子目录
    let git_subdir = repo_path.join(".git");
    if git_subdir.is_dir() && git_subdir.join("HEAD").exists() {
        return Ok(git_subdir);
    }
    Err(format!(
        "无效的 Git 仓库路径: {}（未找到 .git 目录或 bare 仓库结构）",
        repo_path.display()
    ))
}

/// 从 Git 仓库导出指定分支
fn collect_git(
    repo_path: &std::path::Path,
    branch: &str,
    archive_name: &str,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    if !repo_path.exists() {
        return Err(format!("Git 仓库不存在: {}", repo_path.display()));
    }

    let git_dir = resolve_git_dir(repo_path)?;

    let dest_dir = temp_dir.join(archive_name);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("创建目标目录失败: {e}"))?;

    let output_file = dest_dir.join(format!("{archive_name}.tar.gz"));

    let mut cmd = Command::new("git");
    cmd.arg("--git-dir")
        .arg(&git_dir)
        .arg("archive")
        .arg("--format=tar.gz")
        .arg(format!("--prefix={archive_name}/"))
        .arg(branch);

    // 将 git archive 的输出重定向到文件
    let child = cmd
        .stdout(std::fs::File::create(&output_file).map_err(|e| {
            format!("创建输出文件失败: {e}")
        })?)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行 git archive 失败: {e}"))?;

    let output = child.wait_with_output().map_err(|e| format!("等待进程失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        fs::remove_file(&output_file).ok();
        return Err(format!(
            "git archive 失败 (branch={branch}): {stderr}"
        ));
    }

    Ok(format!("已导出 Git 仓库: {archive_name} (branch: {branch})"))
}

/// 清理临时目录
pub fn cleanup_temp_dir(temp_dir: &PathBuf) {
    if temp_dir.exists() {
        tracing::info!("正在清理临时目录...");
        let _ = fs::remove_dir_all(temp_dir);
    }
}
