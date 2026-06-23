#![warn(clippy::all)]

mod app;
mod backup_entry;
mod backup_runner;
mod builder;
mod compressor;
mod config;
mod project_backup;
mod ui;
mod validation;

use app::BackupApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 750.0])
            .with_min_inner_size([680.0, 520.0])
            .with_title("备份工具 - Backup Utility"),
        ..Default::default()
    };

    eframe::run_native(
        "备份工具",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            setup_theme(&cc.egui_ctx);
            Ok(Box::new(BackupApp::new(cc)))
        }),
    )
}

/// 配置中文字体 — 运行时从系统加载，避免嵌入二进制
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 尝试加载系统自带的中文字体（Windows 常见路径）
    let system_font_paths = [
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",      // 微软雅黑
        "C:\\Windows\\Fonts\\msyhbd.ttc",  // 微软雅黑粗体
        "C:\\Windows\\Fonts\\simsun.ttc",  // 宋体
        "C:\\Windows\\Fonts\\simhei.ttf",  // 黑体
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Linux
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];

    let mut loaded = false;
    for path in &system_font_paths {
        if let Ok(font_data) = std::fs::read(path) {
            let name = format!("system_cjk_{}", loaded);
            fonts.font_data.insert(
                name.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(font_data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            loaded = true;
            tracing::info!("加载系统字体: {}", path);
            break; // 加载一个就够了
        }
    }

    // 如果系统字体都失败，回退到嵌入字体（开发/便携场景）
    if !loaded {
        #[cfg(feature = "embedded-font")]
        {
            const FONT_BYTES: &[u8] = include_bytes!("../assets/NotoSansSC-Subset.ttf");
            fonts.font_data.insert(
                "noto_sans_sc_subset".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(FONT_BYTES)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("noto_sans_sc_subset".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("noto_sans_sc_subset".to_owned());
            tracing::warn!("未找到系统字体，使用嵌入字体回退");
        }
        #[cfg(not(feature = "embedded-font"))]
        {
            tracing::warn!("未找到系统字体，界面可能显示为方框或乱码");
        }
    }

    ctx.set_fonts(fonts);
}

/// 配置全局主题样式
fn setup_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // 间距与内边距
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.indent = 16.0;

    // 圆角
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(6.0);
    style.visuals.widgets.open.rounding = egui::Rounding::same(6.0);

    // 窗口圆角
    style.visuals.window_rounding = egui::Rounding::same(10.0);

    ctx.set_style(style);
}
