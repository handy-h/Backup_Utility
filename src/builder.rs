use std::path::Path;
use std::process::Command;

/// 二进制产物构建器，自动构建缺失/过期的构建产物
pub struct ArtifactBuilder;

impl ArtifactBuilder {
    /// 构建缺失/过期的二进制产物
    /// - macOS: 调用 `make all`
    /// - Windows: 调用 `powershell -File build.ps1`
    pub fn build_missing(base_path: &Path) -> Result<(), String> {
        let os = std::env::consts::OS;
        match os {
            "macos" => Self::run_make(base_path),
            "windows" => Self::run_powershell(base_path),
            _ => Err(format!("不支持的操作系统: {}", os)),
        }
    }

    fn run_make(base_path: &Path) -> Result<(), String> {
        let makefile = base_path.join("Makefile");
        if !makefile.exists() {
            return Err("Makefile 不存在，无法执行构建".to_string());
        }
        let output = Command::new("make")
            .arg("all")
            .current_dir(base_path)
            .output()
            .map_err(|e| format!("执行 make 失败: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("make 构建失败:\n{}", stderr.trim()));
        }
        Ok(())
    }

    fn run_powershell(base_path: &Path) -> Result<(), String> {
        let script = base_path.join("build.ps1");
        if !script.exists() {
            return Err("build.ps1 不存在，无法执行构建".to_string());
        }
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-File")
            .arg(&script)
            .current_dir(base_path)
            .output()
            .map_err(|e| format!("执行 build.ps1 失败: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("build.ps1 构建失败:\n{}", stderr.trim()));
        }
        Ok(())
    }
}
