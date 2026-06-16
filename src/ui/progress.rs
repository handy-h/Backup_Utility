use crate::backup_runner::{BackupStatus, LogMessage, LogLevel};
use eframe::egui;

/// 渲染日志/进度面板
pub fn render_progress(
    ui: &mut egui::Ui,
    logs: &[LogMessage],
    status: &BackupStatus,
) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());

        ui.heading(egui::RichText::new("执行日志").size(16.0).strong());
        ui.add_space(6.0);

        // 状态指示器
        let (status_text, status_color) = match status {
            BackupStatus::Idle => ("● 就绪", egui::Color32::from_rgb(130, 130, 130)),
            BackupStatus::Collecting => ("◉ 正在收集文件...", egui::Color32::from_rgb(66, 133, 244)),
            BackupStatus::Compressing => ("◉ 正在压缩...", egui::Color32::from_rgb(66, 133, 244)),
            BackupStatus::Moving => ("◉ 正在移动文件...", egui::Color32::from_rgb(66, 133, 244)),
            BackupStatus::Done => ("✔ 备份完成", egui::Color32::from_rgb(46, 125, 50)),
            BackupStatus::Error(_) => ("✖ 发生错误", egui::Color32::from_rgb(220, 53, 69)),
        };

        ui.horizontal(|ui| {
            ui.colored_label(status_color, egui::RichText::new(status_text).strong().size(14.0));

            // 运行中的状态显示进度条
            if matches!(status, BackupStatus::Compressing) {
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .animate(true)
                        .desired_width(ui.available_width() - 20.0),
                );
            }
        });

        // 错误详情单独显示
        if let BackupStatus::Error(msg) = status {
            ui.add_space(2.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 53, 69),
                egui::RichText::new(format!("  {msg}")).size(12.0),
            );
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // 日志条目数提示
        if !logs.is_empty() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::GRAY,
                    egui::RichText::new(format!("共 {} 条日志", logs.len())).size(11.0).italics(),
                );
            });
            ui.add_space(2.0);
        }

        // 日志滚动区域
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                egui::Grid::new("log_grid")
                    .spacing([4.0, 2.0])
                    .show(ui, |ui| {
                        for log in logs.iter() {
                            let (icon, color) = match log.level {
                                LogLevel::Info => ("ℹ", egui::Color32::from_rgb(66, 133, 244)),
                                LogLevel::Success => ("✓", egui::Color32::from_rgb(46, 125, 50)),
                                LogLevel::Warn => ("⚠", egui::Color32::from_rgb(255, 180, 0)),
                                LogLevel::Error => ("✗", egui::Color32::from_rgb(220, 53, 69)),
                            };

                            ui.colored_label(
                                egui::Color32::from_rgb(140, 140, 140),
                                egui::RichText::new(&log.timestamp).monospace().size(11.5),
                            );
                            ui.colored_label(color, egui::RichText::new(icon).size(12.0));
                            ui.colored_label(
                                color,
                                egui::RichText::new(&log.message).size(12.0),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}
