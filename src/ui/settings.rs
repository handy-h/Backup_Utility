use crate::compressor::check_compressor_available;
use crate::config::{BackupConfig, CompressorType};
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;

/// UI 侧的编辑状态（不持久化）
#[derive(Default)]
pub struct EditState {
    /// 当前正在编辑的条目索引
    pub editing_index: Option<usize>,
    /// 密码输入缓冲区
    pub password_buffer: String,
}


/// 渲染压缩设置面板
pub fn render_compression_settings(
    ui: &mut egui::Ui,
    config: &mut BackupConfig,
    edit_state: &mut EditState,
) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());

        ui.heading(egui::RichText::new("压缩设置").size(16.0).strong());
        ui.add_space(6.0);

        // 第一行：压缩工具 + 路径
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("工具:").strong());
            let prev_compressor = config.compressor;
            egui::ComboBox::from_id_salt("compressor_type")
                .selected_text(
                    egui::RichText::new(config.compressor.label()).strong(),
                )
                .show_ui(ui, |ui| {
                    for ct in CompressorType::all() {
                        ui.selectable_value(
                            &mut config.compressor,
                            *ct,
                            ct.label(),
                        );
                    }
                });

            // 切换压缩工具时，clamp 压缩级别到合法范围
            if config.compressor != prev_compressor {
                let max = config.compressor.max_level();
                if config.compression_level > max {
                    config.compression_level = max;
                }
            }

            // 显示压缩工具可用性状态
            match check_compressor_available(&config.compressor, &config.compressor_path) {
                Ok(path) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(46, 125, 50),
                        egui::RichText::new("[可用]").size(12.0),
                    );
                    if path != PathBuf::from(config.compressor.default_command()) {
                        ui.label(
                            egui::RichText::new(format!("({})", path.display()))
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    }
                }
                Err(_) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 53, 69),
                        egui::RichText::new("[未找到]").size(12.0),
                    );
                }
            }

            ui.add_space(12.0);
            ui.label(egui::RichText::new("路径:").strong());
            let mut path_str = config
                .compressor_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let response = ui.add_sized(
                [180.0, 22.0],
                egui::TextEdit::singleline(&mut path_str).hint_text("默认(自动查找)"),
            );
            if response.changed() {
                if path_str.trim().is_empty() {
                    config.compressor_path = None;
                } else {
                    config.compressor_path = Some(PathBuf::from(&path_str));
                }
            }
            if ui.button("浏览").clicked()
                && let Some(path) = FileDialog::new()
                    .set_title("选择压缩工具可执行文件")
                    .pick_file()
                {
                    config.compressor_path = Some(path);
                }

            ui.add_space(12.0);
            ui.label(egui::RichText::new("级别:").strong());
            let max = config.compressor.max_level();
            ui.add(egui::Slider::new(&mut config.compression_level, 0..=max));
        });

        // 加密警告提示
        if !config.compressor.supports_password() && config.password.is_some() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 0),
                    egui::RichText::new("⚠").size(14.0),
                );
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 0),
                    format!(
                        "{} 格式不支持密码加密，设置密码后执行将报错",
                        config.compressor.label()
                    ),
                );
            });
        }
        if config.password.is_some()
            && !config.compressor.supports_encrypt_filenames()
            && config.encrypt_filenames
        {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 0),
                    egui::RichText::new("⚠").size(14.0),
                );
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 0),
                    format!(
                        "{} 格式不支持文件名加密，将退回普通密码加密",
                        config.compressor.label()
                    ),
                );
            });
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // 密码设置
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("密码:").strong());
            let pw_response = ui.add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut edit_state.password_buffer).password(true),
            );
            if pw_response.changed() {
                if edit_state.password_buffer.is_empty() {
                    config.password = None;
                } else {
                    config.password = Some(edit_state.password_buffer.clone());
                }
            }
            ui.colored_label(
                egui::Color32::GRAY,
                egui::RichText::new("(留空则不加密)").italics(),
            );
        });

        // 文件名加密选项
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let enabled = config.password.is_some() && config.compressor.supports_encrypt_filenames();
            ui.add_enabled(enabled, egui::Checkbox::new(
                &mut config.encrypt_filenames,
                egui::RichText::new("加密文件名（不输入密码无法查看包内文件名）"),
            ));
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // 输出文件名模板
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("文件名:").strong());
            ui.add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut config.output_filename_pattern),
            );
            ui.colored_label(
                egui::Color32::GRAY,
                egui::RichText::new("使用 {date} 作为日期占位符").italics(),
            );
        });

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(100, 180, 255),
                egui::RichText::new("输出预览:").strong(),
            );
            ui.monospace(crate::backup_runner::preview_output_filename(config));
        });
    });
}

/// 渲染路径设置面板
pub fn render_path_settings(ui: &mut egui::Ui, config: &mut BackupConfig) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());

        ui.heading(egui::RichText::new("路径设置").size(16.0).strong());
        ui.add_space(6.0);

        // 临时目录
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("临时目录:").strong());
            let mut temp_str = config.temp_dir.display().to_string();
            let response = ui.add_sized(
                [300.0, 22.0],
                egui::TextEdit::singleline(&mut temp_str),
            );
            if response.changed() {
                config.temp_dir = PathBuf::from(&temp_str);
            }
            if ui.button("浏览").clicked()
                && let Some(path) = FileDialog::new()
                    .set_title("选择临时文件存放目录")
                    .pick_folder()
                {
                    config.temp_dir = path;
                }
        });

        ui.add_space(2.0);

        // 输出目录
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("输出目录:").strong());
            let mut output_str = config.output_dir.display().to_string();
            let response = ui.add_sized(
                [300.0, 22.0],
                egui::TextEdit::singleline(&mut output_str),
            );
            if response.changed() {
                config.output_dir = PathBuf::from(&output_str);
            }
            if ui.button("浏览").clicked()
                && let Some(path) = FileDialog::new()
                    .set_title("选择压缩包输出目录")
                    .pick_folder()
                {
                    config.output_dir = path;
                }
        });
    });
}
