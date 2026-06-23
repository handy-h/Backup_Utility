use crate::backup_runner::{BackupStatus, LogMessage};
use crate::config::BackupConfig;
use crate::project_backup::ProjectBackup;
use crate::ui::settings::EditState;
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::sync::mpsc;

/// 渲染项目备份界面
pub fn render_project_backup(
    ui: &mut egui::Ui,
    project_backup: &mut ProjectBackup,
    config: &mut BackupConfig,
    edit_state: &mut EditState,
    is_running: &mut bool,
    backup_status: &mut BackupStatus,
    _log_receiver: &mut Option<mpsc::Receiver<LogMessage>>,
    _status_receiver: &mut Option<mpsc::Receiver<BackupStatus>>,
    _logs: &mut Vec<LogMessage>,
) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());

        ui.heading(
            egui::RichText::new("备份当前项目")
                .size(18.0)
                .strong(),
        );
        ui.add_space(8.0);

        // 源路径选择
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("项目路径:").strong());
            let mut source_str = project_backup.source_path.display().to_string();
            let response = ui.add_sized(
                [400.0, 22.0],
                egui::TextEdit::singleline(&mut source_str),
            );
            if response.changed() {
                let new_path = PathBuf::from(&source_str);
                project_backup.source_path = new_path.clone();
                config.project_backup_source = new_path;
            }
            if ui.button("浏览").clicked() {
                if let Some(path) = FileDialog::new()
                    .set_title("选择要备份的项目目录")
                    .pick_folder()
                {
                    project_backup.source_path = path.clone();
                    config.project_backup_source = path;
                }
            }
        });

        ui.add_space(4.0);

        // 目标路径选择
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("目标路径:").strong());
            let mut target_str = project_backup.target_path.display().to_string();
            let response = ui.add_sized(
                [400.0, 22.0],
                egui::TextEdit::singleline(&mut target_str),
            );
            if response.changed() {
                let new_path = PathBuf::from(&target_str);
                project_backup.target_path = new_path.clone();
                config.project_backup_target = new_path;
            }
            if ui.button("浏览").clicked() {
                if let Some(path) = FileDialog::new()
                    .set_title("选择备份目标目录")
                    .pick_folder()
                {
                    project_backup.target_path = path.clone();
                    config.project_backup_target = path;
                }
            }
        });

        ui.add_space(4.0);

        // 压缩包名称
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("压缩包名称:").strong());
            let response = ui.add_sized(
                [250.0, 22.0],
                egui::TextEdit::singleline(&mut project_backup.archive_name),
            );
            if response.changed() {
                config.project_backup_archive_name = project_backup.archive_name.clone();
            }
            ui.colored_label(
                egui::Color32::GRAY,
                egui::RichText::new("(不含扩展名)").italics(),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // 压缩设置
        ui.heading(
            egui::RichText::new("压缩设置")
                .size(16.0)
                .strong(),
        );
        ui.add_space(6.0);

        // 压缩工具选择
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("压缩工具:").strong());
            egui::ComboBox::from_id_salt("project_compressor_type")
                .selected_text(
                    egui::RichText::new(config.compressor.label()).strong(),
                )
                .show_ui(ui, |ui| {
                    for ct in crate::config::CompressorType::all() {
                        ui.selectable_value(
                            &mut config.compressor,
                            *ct,
                            ct.label(),
                        );
                    }
                });

            ui.add_space(12.0);
            ui.label(egui::RichText::new("压缩级别:").strong());
            let max = config.compressor.max_level();
            ui.add(egui::Slider::new(&mut config.compression_level, 0..=max));
        });

        ui.add_space(6.0);

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

        ui.add_space(2.0);

        // 文件名加密选项
        ui.horizontal(|ui| {
            let enabled = config.password.is_some()
                && config.compressor.supports_encrypt_filenames();
            ui.add_enabled(
                enabled,
                egui::Checkbox::new(
                    &mut config.encrypt_filenames,
                    egui::RichText::new("加密文件名"),
                ),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // 排除设置
        ui.heading(
            egui::RichText::new("排除设置")
                .size(16.0)
                .strong(),
        );
        ui.add_space(4.0);

        ui.label(egui::RichText::new("排除的目录/文件（匹配相对路径中任意一级名称）").size(13.0));
        ui.add_space(2.0);

        // 添加排除模式 — 输入 + 浏览目录 + 浏览文件
        ui.horizontal(|ui| {
            let input = ui.add_sized(
                [180.0, 22.0],
                egui::TextEdit::singleline(&mut edit_state.exclude_buffer)
                    .hint_text("手动输入名称"),
            );
            let add_btn = egui::Button::new(
                egui::RichText::new("添加").strong(),
            );
            if ui.add_enabled(!edit_state.exclude_buffer.trim().is_empty(), add_btn).clicked() {
                let pattern = edit_state.exclude_buffer.trim().to_string();
                if !pattern.is_empty() && !config.project_exclude_patterns.contains(&pattern) {
                    config.project_exclude_patterns.push(pattern);
                }
                edit_state.exclude_buffer.clear();
            }
            if input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let pattern = edit_state.exclude_buffer.trim().to_string();
                if !pattern.is_empty() && !config.project_exclude_patterns.contains(&pattern) {
                    config.project_exclude_patterns.push(pattern);
                }
                edit_state.exclude_buffer.clear();
            }

            ui.add_space(8.0);

            // 浏览目录按钮
            if ui.button("浏览目录").clicked() {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title("选择要排除的目录")
                    .set_directory(&project_backup.source_path)
                    .pick_folder()
                {
                    if let Ok(rel) = dir.strip_prefix(&project_backup.source_path) {
                        let pattern = rel.to_string_lossy().to_string();
                        if !pattern.is_empty()
                            && !config.project_exclude_patterns.contains(&pattern)
                        {
                            config.project_exclude_patterns.push(pattern);
                        }
                    }
                }
            }

            // 浏览文件按钮
            if ui.button("浏览文件").clicked() {
                if let Some(file) = rfd::FileDialog::new()
                    .set_title("选择要排除的文件")
                    .set_directory(&project_backup.source_path)
                    .pick_file()
                {
                    if let Ok(rel) = file.strip_prefix(&project_backup.source_path) {
                        let pattern = rel.to_string_lossy().to_string();
                        if !pattern.is_empty()
                            && !config.project_exclude_patterns.contains(&pattern)
                        {
                            config.project_exclude_patterns.push(pattern);
                        }
                    }
                }
            }
        });

        ui.add_space(4.0);

        // 已有排除模式列表
        let mut remove_idx: Option<usize> = None;
        if config.project_exclude_patterns.is_empty() {
            ui.colored_label(
                egui::Color32::GRAY,
                egui::RichText::new("  暂无自定义排除规则").italics().size(12.0),
            );
        } else {
            let mut idx = 0;
            while idx < config.project_exclude_patterns.len() {
                let pattern = &config.project_exclude_patterns[idx];
                let is_file = pattern.contains('.') && !pattern.ends_with('/');
                ui.horizontal(|ui| {
                    let icon = if is_file { "📄" } else { "📁" };
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 100, 50),
                        egui::RichText::new(format!("✗ {} ", icon)).size(14.0),
                    );
                    ui.label(
                        egui::RichText::new(pattern).size(13.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("删除").clicked() {
                            remove_idx = Some(idx);
                        }
                    });
                });
                idx += 1;
            }
        }

        if let Some(idx) = remove_idx {
            config.project_exclude_patterns.remove(idx);
        }

        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("提示: 可手动输入名称，或用「浏览目录」「浏览文件」选择，添加后将在备份时跳过")
                .size(11.0)
                .color(egui::Color32::GRAY),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // 显示配置验证错误
        if let Err(e) = project_backup.validate() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 53, 69),
                    egui::RichText::new("⚠").size(14.0),
                );
                ui.colored_label(
                    egui::Color32::from_rgb(220, 53, 69),
                    format!("配置错误: {e}"),
                );
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // 显示当前状态
        if *is_running {
            ui.horizontal(|ui| {
                match backup_status {
                    BackupStatus::Collecting => {
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 180, 255),
                            "正在准备文件...",
                        );
                    }
                    BackupStatus::Compressing => {
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 180, 255),
                            "正在压缩...",
                        );
                    }
                    _ => {}
                }
            });
            ui.add_space(4.0);
        }
    });
}
