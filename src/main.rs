#![warn(clippy::all)]

mod app;
mod backup_entry;
mod backup_runner;
mod compressor;
mod config;
mod ui;

use app::BackupApp;
use eframe::egui;

/// 嵌入中文字体子集（Noto Sans SC Subset）
const FONT_BYTES: &[u8] = include_bytes!("../assets/NotoSansSC-Subset.ttf");

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

/// 配置中文字体
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 注册中文字体
    fonts.font_data.insert(
        "noto_sans_sc_subset".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_BYTES)),
    );

    // 将中文字体添加到 Proportional 字体族的末尾（作为回退）
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("noto_sans_sc_subset".to_owned());

    // 将中文字体添加到 Monospace 字体族的末尾（作为回退）
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("noto_sans_sc_subset".to_owned());

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
