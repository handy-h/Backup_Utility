#!/usr/bin/env bash
# ============================================================
# 自动备份脚本 - 业务知识库
# 用途: 将本地多个应用数据分类打包为加密 7z 压缩包
# 最后修改: 2026-05-14
# 依赖: 7z (p7zip), git
# ============================================================

set -euo pipefail

# --- 1. 基础路径配置 ---
BASE_DIR="/Users/mengshu/业务知识"
RAM_DISK="/Volumes/RamDisk"
FINAL_DEST="${BASE_DIR}/备份"
BACKUP_DATE=$(date +%Y%m%d)
TARGET_NAME="backup${BACKUP_DATE}.7z"
ASHEN_BARE_REPO="/Users/mengshu/git_depot/Ashen_Protocol/Ashen_protocol.git"

# --- 2. 日志工具函数 ---
log_info()  { echo "[$(date '+%H:%M:%S')] ℹ $*"; }
log_ok()    { echo "[$(date '+%H:%M:%S')] ✓ $*"; }
log_warn()  { echo "[$(date '+%H:%M:%S')] ⚠ $*"; }
log_error() { echo "[$(date '+%H:%M:%S')] ✗ $*" >&2; }

# --- 3. 清理函数与 trap 设置 ---
TEMP_DIR=""

cleanup() {
    local exit_code=$?
    if [[ -n "$TEMP_DIR" && -d "$TEMP_DIR" ]]; then
        log_info "正在清理 RAM Disk 临时目录..."
        rm -rf "$TEMP_DIR"
    fi
    if [[ $exit_code -ne 0 ]]; then
        log_error "脚本执行出错，退出码: $exit_code"
    fi
    exit "$exit_code"
}

trap cleanup EXIT INT TERM

# --- 4. 安全获取密码 ---
# [改动] 增加空密码校验
echo "请输入备份加密密码 (输入时屏幕不会显示):"
read -rs MY_PASSWORD
echo ""

if [[ -z "$MY_PASSWORD" ]]; then
    log_error "密码不能为空，退出。"
    exit 1
fi

log_info "密码已记录，开始处理..."

# --- 5. 前置检查 ---
# 检查 7z 命令是否存在
if ! command -v 7z &> /dev/null; then
    log_error "未找到 7z 命令，请先安装 p7zip！"
    exit 1
fi

# 检查 RAM Disk 是否挂载
if [[ ! -d "$RAM_DISK" ]]; then
    log_error "RAM Disk 未挂载，请先创建内存盘！"
    exit 1
fi

# 检查 git 命令是否存在
if ! command -v git &> /dev/null; then
    log_error "未找到 git 命令！"
    exit 1
fi

# 检查 Ashen_protocol bare 仓库
if [[ ! -d "$ASHEN_BARE_REPO" ]]; then
    log_warn "Ashen_protocol bare 仓库不存在: $ASHEN_BARE_REPO"
    log_warn "备份将跳过 Ashen_protocol 项目"
fi

# [改动] cd 失败时输出错误信息
if ! cd "$BASE_DIR"; then
    log_error "无法切换到工作目录: $BASE_DIR"
    exit 1
fi

# --- 6. 通用安全复制函数 ---
safe_cp() {
    local src="$1"
    local dest_dir="$2"
    local recursive="${3:-false}"

    if [[ -z "$src" ]]; then
        return 0
    fi

    if [[ "$recursive" == "true" && -d "$src" ]]; then
        cp -R "$src" "$dest_dir/"
        log_ok "已复制目录: $(basename "$src")"
    elif [[ "$recursive" != "true" && -f "$src" ]]; then
        cp "$src" "$dest_dir/"
        log_ok "已复制文件: $(basename "$src")"
    else
        log_warn "跳过不存在的项目: $src"
    fi
}

# --- 7. 查找最新 HTML 文件 ---
LATEST_HTML_FULL=""
if ls "${BASE_DIR}/备份"/*.html &> /dev/null; then
    LATEST_HTML_FULL=$(ls -t "${BASE_DIR}/备份"/*.html | head -n 1)
fi

# [改动] 空值时给出日志提示
if [[ -z "$LATEST_HTML_FULL" ]]; then
    log_warn "未找到任何 HTML 书签文件，跳过 bookmark 备份"
    LATEST_HTML_REL=""
else
    LATEST_HTML_REL="${LATEST_HTML_FULL#${BASE_DIR}/}"
fi

# --- 8. 创建分类目录结构 ---
TEMP_DIR="${RAM_DISK}/backup_${BACKUP_DATE}"
mkdir -p "${TEMP_DIR}"/{bookmark,rime,obsidian,keepass,apipost}

# [改动] 创建后验证
if [[ ! -d "$TEMP_DIR" ]]; then
    log_error "无法创建临时目录: $TEMP_DIR"
    exit 1
fi

log_info "正在按软件来源分类整理文件..."

# --- 9. 按软件来源分类复制文件 ---
# 1. bookmark: 存放最新 HTML 文件
safe_cp "$LATEST_HTML_REL" "${TEMP_DIR}/bookmark"

# 2. rime: 原 rime 目录
safe_cp "rime" "${TEMP_DIR}/rime" true

# 3. obsidian: 更新为正确路径
OBSIDIAN_SRC="${HOME}/Builds/我的知识库"
if [[ -d "$OBSIDIAN_SRC" ]]; then
    log_info "正在复制 Obsidian 知识库..."
    mkdir -p "${TEMP_DIR}/obsidian"
    cp -R "$OBSIDIAN_SRC"/* "${TEMP_DIR}/obsidian/" || true
    # 清理不需要的文件 (历史记录/缓存)
    # [改动] -prune 避免已知竞态问题
    find "${TEMP_DIR}/obsidian" -name ".DS_Store" -delete
    find "${TEMP_DIR}/obsidian" -path "*/copilot/history" -type d -prune -exec rm -rf {} + 2>/dev/null || true
    find "${TEMP_DIR}/obsidian" -path "*/copilot/cache" -type d -prune -exec rm -rf {} + 2>/dev/null || true
    find "${TEMP_DIR}/obsidian" -name "*.log" -delete
fi

# 4. keepass: 存放原 keepass/handy.kdbx
safe_cp "keepass/handy.kdbx" "${TEMP_DIR}/keepass"

# 5. Ashen_protocol: 从 bare 仓库导出 main 分支打包
# [改动] 移除重复的 ASHEN_BARE_REPO 定义，使用顶部统一变量
if [[ -d "$ASHEN_BARE_REPO" ]]; then
    log_info "正在从 Ashen_protocol 仓库导出 main 分支..."
    mkdir -p "${TEMP_DIR}/Ashen_protocol"
    # [改动] 增加 git archive 错误处理
    if git --git-dir="$ASHEN_BARE_REPO" archive \
        --format=tar.gz --prefix=Ashen_protocol/ main \
        > "${TEMP_DIR}/Ashen_protocol/Ashen_protocol.tar.gz"; then
        log_ok "已生成 Ashen_protocol.tar.gz"
    else
        log_error "git archive 失败，请检查 main 分支是否存在"
        rm -f "${TEMP_DIR}/Ashen_protocol/Ashen_protocol.tar.gz"
    fi
fi

# 7. apipost
safe_cp "阿斯忒瑞亚的私有项目.json" "${TEMP_DIR}/apipost"

# --- 10. 在 RAM Disk 中进行极限压缩 ---
log_info "正在 RAM Disk 中进行极限压缩 (-mx=9)..."
7z a "${RAM_DISK}/${TARGET_NAME}" \
    "${TEMP_DIR}" \
    -p"${MY_PASSWORD}" \
    -mhe=on \
    -mx=9 \
    -ms=on \
    -xr!'.DS_Store' \
    -xr!.qoder \
    -xr!.vscode \
    -xr!"node_modules"

# --- 11. 移动到最终目的地 ---
# [改动] 直接用 if 判断 mv 的返回值，修复 $? 死代码问题
log_info "正在移动到备份目录..."
if mv "${RAM_DISK}/${TARGET_NAME}" "$FINAL_DEST/"; then
    log_info "-------------------------------------------"
    log_info "✅ 备份成功！"
    log_info "最终位置: ${FINAL_DEST}/${TARGET_NAME}"
else
    log_error "移动失败，请检查磁盘空间。"
    exit 1
fi
