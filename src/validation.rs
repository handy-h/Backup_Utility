use std::path::Path;
use std::fs;

/// 文件验证器，检查项目必需文件、目录和可执行文件
pub struct FileValidator {
    /// 是否包含 AI 技能模块
    pub include_skills: bool,
}

impl FileValidator {
    pub fn new(include_skills: bool) -> Self {
        Self { include_skills }
    }

    /// 验证运行必需文件列表
    pub fn validate_essential(&self, base_path: &Path) -> Result<(), String> {
        // 基础配置文件
        let basic_files = &[".env", "Makefile", "build.ps1"];
        for f in basic_files {
            self.require_file(&base_path.join(f))?;
        }

        // 关键配置文件
        self.require_file(&base_path.join("website/config/manifest.ash"))?;

        // 前端静态资源
        self.require_file(&base_path.join("website/frontend/index.html"))?;
        self.require_dir(&base_path.join("website/frontend/css"))?;
        self.require_dir(&base_path.join("website/frontend/js"))?;

        // 可选技能模块
        if self.include_skills {
            self.require_dir(&base_path.join("modules/skills"))?;
        }

        Ok(())
    }

    /// 验证平台二进制文件
    pub fn validate_binaries(&self, base_path: &Path) -> Result<(), String> {
        let os = std::env::consts::OS;
        match os {
            "macos" => {
                for arch in &["arm64", "amd64"] {
                    let protocol = base_path.join(format!("dist/darwin-{}/Ashen-Protocol", arch));
                    self.require_executable(&protocol)?;
                    let tools = base_path.join(format!("dist/darwin-{}/ashen-tools", arch));
                    self.require_executable(&tools)?;
                }
            }
            "windows" => {
                for exe in &["Ashen-Protocol.exe", "ashen-tools.exe"] {
                    self.require_executable(
                        &base_path.join(format!("dist/windows-amd64/{}", exe)),
                    )?;
                }
            }
            _ => return Err(format!("不支持的操作系统: {}", os)),
        }
        Ok(())
    }

    /// 检查文件必须存在且是普通文件
    pub fn require_file(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("必需文件不存在: {}", path.display()));
        }
        if !path.is_file() {
            return Err(format!("路径不是文件: {}", path.display()));
        }
        Ok(())
    }

    /// 检查目录必须存在
    pub fn require_dir(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("必需目录不存在: {}", path.display()));
        }
        if !path.is_dir() {
            return Err(format!("路径不是目录: {}", path.display()));
        }
        Ok(())
    }

    /// 检查可执行文件必须存在且具有执行权限
    pub fn require_executable(&self, path: &Path) -> Result<(), String> {
        self.require_file(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(path).map_err(|e| e.to_string())?;
            if meta.permissions().mode() & 0o111 == 0 {
                return Err(format!("文件不可执行: {}", path.display()));
            }
        }
        Ok(())
    }
}
