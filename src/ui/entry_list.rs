use crate::config::BackupEntry;
use crate::ui::settings::EditState;
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;

/// 渲染备份条目列表
pub fn render_entry_list(
    ui: &mut egui::Ui,
    entries: &mut Vec<BackupEntry>,
    edit_state: &mut EditState,
) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());

        // 标题栏
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("备份条目").size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let del_btn = egui::Button::new(
                    egui::RichText::new("删除选中").color(egui::Color32::from_rgb(220, 53, 69)),
                )
                .rounding(egui::Rounding::same(4.0));

                if ui.add(del_btn).clicked()
                    && let Some(idx) = edit_state.editing_index
                {
                    if idx < entries.len() {
                        entries.remove(idx);
                    }
                    edit_state.editing_index = None;
                }

                let add_dir_btn = egui::Button::new(
                    egui::RichText::new("+ 添加目录").strong(),
                )
                .rounding(egui::Rounding::same(4.0));

                if ui.add(add_dir_btn).clicked() {
                    add_local_dir_entry(entries, edit_state);
                }

                let add_file_btn = egui::Button::new(
                    egui::RichText::new("+ 添加文件").strong(),
                )
                .rounding(egui::Rounding::same(4.0));

                if ui.add(add_file_btn).clicked() {
                    add_local_file_entry(entries, edit_state);
                }

                let add_git_btn = egui::Button::new(
                    egui::RichText::new("+ 添加 Git 仓库").strong(),
                )
                .rounding(egui::Rounding::same(4.0));

                if ui.add(add_git_btn).clicked() {
                    add_git_entry(entries, edit_state);
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if entries.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.colored_label(
                    egui::Color32::GRAY,
                    egui::RichText::new("暂无备份条目，点击上方按钮添加").size(14.0),
                );
            });
            ui.add_space(12.0);
        } else {
            // 表头 — 5 列：类型 | 源路径 | 分支 | 包内名称 | 操作
            egui::Grid::new("entry_grid")
                .striped(true)
                .spacing([8.0, 6.0])
                .min_col_width(50.0)
                .show(ui, |ui| {
                    // 表头样式
                    ui.label(egui::RichText::new("类型").strong().size(13.0));
                    ui.label(egui::RichText::new("源路径").strong().size(13.0));
                    ui.label(egui::RichText::new("分支").strong().size(13.0));
                    ui.label(egui::RichText::new("包内名称").strong().size(13.0));
                    ui.label(egui::RichText::new("操作").strong().size(13.0));
                    ui.end_row();

                    // 先收集需要删除的索引
                    let mut remove_idx = None;

                    for (i, entry) in entries.iter_mut().enumerate() {
                        let is_editing = edit_state.editing_index == Some(i);

                        // —— 列1: 类型 ——
                        let (entry_type, type_color) = match entry {
                            BackupEntry::LocalFile { .. } => (
                                "本地",
                                egui::Color32::from_rgb(66, 133, 244),
                            ),
                            BackupEntry::GitArchive { .. } => (
                                "Git",
                                egui::Color32::from_rgb(234, 67, 53),
                            ),
                        };
                        ui.colored_label(type_color, egui::RichText::new(entry_type).strong());

                        if is_editing {
                            // —— 编辑模式 ——
                            match entry {
                                BackupEntry::LocalFile {
                                    source_path,
                                    archive_name,
                                } => {
                                    // 列2: 源路径（编辑）
                                    let mut path_str = source_path.display().to_string();
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [180.0, 22.0],
                                            egui::TextEdit::singleline(&mut path_str),
                                        );
                                        if ui.button("选目录").clicked()
                                            && let Some(path) = FileDialog::new()
                                                .set_title("选择目录")
                                                .pick_folder()
                                        {
                                            *source_path = path;
                                        }
                                        if ui.button("选文件").clicked()
                                            && let Some(path) = FileDialog::new()
                                                .set_title("选择文件")
                                                .pick_file()
                                        {
                                            *source_path = path;
                                        }
                                    });
                                    *source_path = PathBuf::from(&path_str);

                                    // 列3: 分支（本地条目无分支，显示占位）
                                    ui.label(egui::RichText::new("-").color(egui::Color32::GRAY));

                                    // 列4: 包内名称（编辑）
                                    ui.add_sized(
                                        [140.0, 22.0],
                                        egui::TextEdit::singleline(archive_name),
                                    );
                                }
                                BackupEntry::GitArchive {
                                    repo_path,
                                    branch,
                                    archive_name,
                                } => {
                                    // 列2: 源路径（编辑）
                                    let mut path_str = repo_path.display().to_string();
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [180.0, 22.0],
                                            egui::TextEdit::singleline(&mut path_str),
                                        );
                                        if ui.button("浏览").clicked()
                                            && let Some(path) = FileDialog::new()
                                                .set_title("选择 Git 仓库")
                                                .pick_folder()
                                        {
                                            *repo_path = path;
                                        }
                                    });
                                    *repo_path = PathBuf::from(&path_str);

                                    // 列3: 分支（编辑）
                                    ui.add_sized(
                                        [100.0, 22.0],
                                        egui::TextEdit::singleline(branch).hint_text("分支名"),
                                    );

                                    // 列4: 包内名称（编辑）
                                    ui.add_sized(
                                        [140.0, 22.0],
                                        egui::TextEdit::singleline(archive_name),
                                    );
                                }
                            }
                        } else {
                            // —— 显示模式 ——
                            match entry {
                                BackupEntry::LocalFile {
                                    source_path,
                                    archive_name,
                                } => {
                                    // 列2: 源路径
                                    ui.label(
                                        egui::RichText::new(source_path.to_string_lossy().as_ref())
                                            .monospace()
                                            .size(12.5),
                                    );
                                    // 列3: 分支（占位）
                                    ui.label(egui::RichText::new("-").color(egui::Color32::GRAY));
                                    // 列4: 包内名称
                                    ui.label(archive_name.as_str());
                                }
                                BackupEntry::GitArchive {
                                    repo_path,
                                    branch,
                                    archive_name,
                                } => {
                                    // 列2: 源路径
                                    ui.label(
                                        egui::RichText::new(repo_path.to_string_lossy().as_ref())
                                            .monospace()
                                            .size(12.5),
                                    );
                                    // 列3: 分支
                                    ui.colored_label(
                                        egui::Color32::from_rgb(100, 180, 255),
                                        egui::RichText::new(branch.as_str()).italics(),
                                    );
                                    // 列4: 包内名称
                                    ui.label(archive_name.as_str());
                                }
                            }
                        }

                        // 列5: 操作按钮
                        ui.horizontal(|ui| {
                            if is_editing {
                                let done_btn = egui::Button::new(
                                    egui::RichText::new("完成").color(egui::Color32::from_rgb(46, 125, 50)),
                                )
                                .rounding(egui::Rounding::same(4.0));
                                if ui.add(done_btn).clicked() {
                                    edit_state.editing_index = None;
                                }
                            } else {
                                if ui.button("编辑").clicked() {
                                    edit_state.editing_index = Some(i);
                                }
                            }
                            let rm_btn = egui::Button::new(
                                egui::RichText::new("移除").color(egui::Color32::from_rgb(200, 80, 80)),
                            )
                            .rounding(egui::Rounding::same(4.0));
                            if ui.add(rm_btn).clicked() {
                                if edit_state.editing_index == Some(i) {
                                    edit_state.editing_index = None;
                                } else if let Some(edit_idx) = edit_state.editing_index
                                    && i < edit_idx
                                {
                                    edit_state.editing_index = Some(edit_idx - 1);
                                }
                                remove_idx = Some(i);
                            }
                        });
                        ui.end_row();
                    }

                    // 延迟删除，避免借用冲突
                    if let Some(idx) = remove_idx {
                        entries.remove(idx);
                    }
                });
        }
    });
}

/// 添加本地文件条目
fn add_local_file_entry(entries: &mut Vec<BackupEntry>, edit_state: &mut EditState) {
    if let Some(path) = FileDialog::new()
        .set_title("选择文件")
        .pick_file()
    {
        let archive_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        entries.push(BackupEntry::LocalFile {
            source_path: path,
            archive_name,
        });
        edit_state.editing_index = Some(entries.len() - 1);
    }
}

/// 添加本地目录条目
fn add_local_dir_entry(entries: &mut Vec<BackupEntry>, edit_state: &mut EditState) {
    if let Some(path) = FileDialog::new()
        .set_title("选择目录")
        .pick_folder()
    {
        let archive_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        entries.push(BackupEntry::LocalFile {
            source_path: path,
            archive_name,
        });
        edit_state.editing_index = Some(entries.len() - 1);
    }
}

/// 添加 Git 仓库条目
fn add_git_entry(entries: &mut Vec<BackupEntry>, edit_state: &mut EditState) {
    if let Some(path) = FileDialog::new()
        .set_title("选择 Git 仓库目录")
        .pick_folder()
    {
        let archive_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "git_repo".to_string());

        entries.push(BackupEntry::GitArchive {
            repo_path: path,
            branch: "main".to_string(),
            archive_name,
        });
        edit_state.editing_index = Some(entries.len() - 1);
    }
}
