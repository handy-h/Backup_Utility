use crate::backup_runner::{run_backup, BackupStatus, LogMessage};
use crate::config::{load_config, save_config, BackupConfig};
use crate::project_backup::ProjectBackup;
use crate::ui::{entry_list, progress, settings, settings::EditState};
use eframe::egui;
use std::sync::mpsc;
use std::thread;

/// 当前激活的标签页
#[derive(Debug, Clone, Copy, PartialEq)]
enum ActiveTab {
    BackupEntries,
    ProjectBackup,
}

/// 待确认的备份任务
struct PendingProjectBackup {
    project_backup: ProjectBackup,
    config: BackupConfig,
    summary: String,
}

/// 主应用结构体
pub struct BackupApp {
    /// 备份配置
    config: BackupConfig,
    /// UI 编辑状态
    edit_state: EditState,
    /// 备份执行状态
    backup_status: BackupStatus,
    /// 日志消息列表
    logs: Vec<LogMessage>,
    /// 是否正在备份中
    is_running: bool,
    /// 接收后台日志消息的 channel receiver
    log_receiver: Option<mpsc::Receiver<LogMessage>>,
    /// 接收后台状态变更的 channel receiver
    status_receiver: Option<mpsc::Receiver<BackupStatus>>,
    /// 当前激活的标签页
    active_tab: ActiveTab,
    /// 项目备份配置
    project_backup: ProjectBackup,
    /// 待用户确认的项目备份 (验证通过后暂存)
    pending_backup: Option<PendingProjectBackup>,
}

impl BackupApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = load_config();
        tracing::info!("已加载配置 ({} 条备份条目)", config.entries.len());

        let project_backup = ProjectBackup {
            source_path: config.project_backup_source.clone(),
            target_path: config.project_backup_target.clone(),
            archive_name: config.project_backup_archive_name.clone(),
        };

        BackupApp {
            config,
            edit_state: EditState::default(),
            backup_status: BackupStatus::Idle,
            logs: Vec::new(),
            is_running: false,
            log_receiver: None,
            status_receiver: None,
            active_tab: ActiveTab::BackupEntries,
            project_backup,
            pending_backup: None,
        }
    }

    /// 检查后台消息并更新 UI
    fn poll_messages(&mut self) {
        // 接收日志消息
        if let Some(rx) = &self.log_receiver {
            while let Ok(msg) = rx.try_recv() {
                self.logs.push(msg);
                self.logs.truncate(500);
            }
        }

        // 接收状态变更
        let should_stop = if let Some(rx) = &self.status_receiver {
            let mut stop = false;
            loop {
                match rx.try_recv() {
                    Ok(status) => {
                        self.backup_status = status;
                        if self.backup_status.is_finished() {
                            stop = true;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        if self.is_running && !self.backup_status.is_finished() {
                            self.backup_status = BackupStatus::Error(
                                "后台任务异常终止，请检查终端日志".into(),
                            );
                            stop = true;
                        }
                        break;
                    }
                }
            }
            stop
        } else {
            false
        };

        if should_stop {
            self.is_running = false;
            self.log_receiver = None;
            self.status_receiver = None;
        }
    }

    /// 开始备份
    fn start_backup(&mut self) {
        if self.is_running {
            return;
        }

        match self.active_tab {
            ActiveTab::BackupEntries => {
                self.start_entries_backup();
            }
            ActiveTab::ProjectBackup => {
                self.start_project_backup_precheck();
            }
        }
    }

    /// 项目备份：先执行验证和构建，然后请求用户确认
    fn start_project_backup_precheck(&mut self) {
        // 同步密码
        if !self.edit_state.password_buffer.is_empty() {
            self.config.password = Some(self.edit_state.password_buffer.clone());
        }

        // 克隆当前配置用于验证，避免验证期间用户修改
        let pb = self.project_backup.clone();
        let config = self.config.clone();

        // 创建临时通道用于收集验证日志
        let (log_tx, log_rx) = mpsc::channel();

        // 在日志中显示验证开始
        self.logs.push(LogMessage::info("正在验证项目配置..."));

        // 执行验证（在 UI 线程中执行，验证通常很快）
        let compression_level = config.compression_level;
        match pb.validate_project(&log_tx, compression_level, &config.project_exclude_patterns) {
            Ok(summary) => {
                // 收集验证日志
                while let Ok(msg) = log_rx.try_recv() {
                    self.logs.push(msg);
                }

                // 存储待确认的备份任务
                self.pending_backup = Some(PendingProjectBackup {
                    project_backup: pb,
                    config,
                    summary,
                });
            }
            Err(e) => {
                while let Ok(msg) = log_rx.try_recv() {
                    self.logs.push(msg);
                }
                self.logs.push(LogMessage::error(format!("验证失败: {e}")));
            }
        }
    }

    /// 执行已确认的项目备份（启动后台线程）
    fn confirm_project_backup(&mut self) {
        if let Some(pending) = self.pending_backup.take() {
            let (log_tx, log_rx) = mpsc::channel();
            let (status_tx, status_rx) = mpsc::channel();

            self.log_receiver = Some(log_rx);
            self.status_receiver = Some(status_rx);
            self.is_running = true;
            self.backup_status = BackupStatus::Collecting;

            self.logs.push(LogMessage::info("开始项目备份..."));
            self.logs.push(LogMessage::info(format!(
                "源路径: {}",
                pending.project_backup.source_path.display()
            )));
            self.logs.push(LogMessage::info(format!(
                "目标路径: {}",
                pending.project_backup.target_path.display()
            )));

            // 在后台线程中执行实际备份
            let pb = pending.project_backup;
            let config = pending.config;
            thread::spawn(move || {
                let result = pb.run_backup(&config, &log_tx);

                match result {
                    Ok(output_path) => {
                        let _ = status_tx.send(BackupStatus::Done);
                        let _ = log_tx.send(LogMessage::info("-------------------------------------------"));
                        let _ = log_tx.send(LogMessage::ok(format!(
                            "项目备份成功！最终位置: {}",
                            output_path.display()
                        )));
                    }
                    Err(e) => {
                        let _ = status_tx.send(BackupStatus::Error(e.clone()));
                        let _ = log_tx.send(LogMessage::error(format!("项目备份失败: {e}")));
                    }
                }
            });
        }
    }

    /// 取消待确认的备份
    fn cancel_pending_backup(&mut self) {
        self.pending_backup = None;
        self.logs
            .push(LogMessage::warn("用户取消了项目备份"));
    }

    /// 开始条目备份
    fn start_entries_backup(&mut self) {
        if !self.edit_state.password_buffer.is_empty() {
            self.config.password = Some(self.edit_state.password_buffer.clone());
        }

        let config = self.config.clone();
        let (log_tx, log_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();

        self.log_receiver = Some(log_rx);
        self.status_receiver = Some(status_rx);
        self.is_running = true;
        self.backup_status = BackupStatus::Collecting;

        thread::spawn(move || {
            run_backup(config, log_tx, status_tx);
        });
    }

    /// 保存配置
    fn save_config(&mut self) {
        if !self.edit_state.password_buffer.is_empty() {
            self.config.password = Some(self.edit_state.password_buffer.clone());
        }
        match save_config(&self.config) {
            Ok(_) => {
                self.logs.push(LogMessage::ok("配置已保存"));
            }
            Err(e) => {
                self.logs.push(LogMessage::error(e));
            }
        }
    }

    /// 清空日志
    fn clear_logs(&mut self) {
        self.logs.clear();
    }
}

impl eframe::App for BackupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 轮询后台消息
        self.poll_messages();

        // 如果正在运行，持续刷新 UI
        if self.is_running {
            ctx.request_repaint();
        }

        // 顶部标题栏（固定）
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("备份工具")
                        .size(22.0)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                    );
                });
            });
            ui.add_space(4.0);
        });

        // 底部操作栏（固定）
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let backup_btn = egui::Button::new(
                    egui::RichText::new("开始备份").strong().color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(46, 125, 50))
                .rounding(egui::Rounding::same(6.0))
                .min_size(egui::vec2(120.0, 32.0));

                if ui
                    .add_enabled(!self.is_running && self.pending_backup.is_none(), backup_btn)
                    .clicked()
                {
                    self.start_backup();
                }

                if self.pending_backup.is_some() {
                    let confirm_btn = egui::Button::new(
                        egui::RichText::new("确认备份")
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(46, 125, 50))
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(120.0, 32.0));

                    if ui.add(confirm_btn).clicked() {
                        self.confirm_project_backup();
                    }

                    let cancel_btn = egui::Button::new(
                        egui::RichText::new("取消")
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(180, 50, 50))
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(100.0, 32.0));

                    if ui.add(cancel_btn).clicked() {
                        self.cancel_pending_backup();
                    }
                }

                let save_btn = egui::Button::new(
                    egui::RichText::new("保存配置").strong(),
                )
                .rounding(egui::Rounding::same(6.0))
                .min_size(egui::vec2(110.0, 32.0));

                if ui.add(save_btn).clicked() {
                    self.save_config();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let clear_btn = egui::Button::new(
                        egui::RichText::new("清空日志"),
                    )
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(100.0, 32.0));

                    if ui.add(clear_btn).clicked() {
                        self.clear_logs();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // 中央内容区（可滚动）
        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab 切换栏
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.active_tab,
                    ActiveTab::BackupEntries,
                    egui::RichText::new("备份条目").strong(),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ActiveTab::ProjectBackup,
                    egui::RichText::new("备份当前项目").strong(),
                );
            });
            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    ActiveTab::BackupEntries => {
                        entry_list::render_entry_list(
                            ui,
                            &mut self.config.entries,
                            &mut self.edit_state,
                        );

                        ui.add_space(10.0);

                        settings::render_compression_settings(
                            ui,
                            &mut self.config,
                            &mut self.edit_state,
                        );

                        ui.add_space(10.0);

                        settings::render_path_settings(ui, &mut self.config);
                    }
                    ActiveTab::ProjectBackup => {
                        crate::ui::project_backup::render_project_backup(
                            ui,
                            &mut self.project_backup,
                            &mut self.config,
                            &mut self.edit_state,
                            &mut self.is_running,
                            &mut self.backup_status,
                            &mut self.log_receiver,
                            &mut self.status_receiver,
                            &mut self.logs,
                        );
                    }
                }

                // 日志/进度面板
                ui.add_space(10.0);
                progress::render_progress(ui, &self.logs, &self.backup_status);
            });
        });

        // 项目备份确认对话框
        if self.pending_backup.is_some() {
            let summary = self.pending_backup.as_ref().unwrap().summary.clone();
            let mut action: Option<&str> = None;

            egui::Window::new("备份确认")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(true)
                .default_size([520.0, 400.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(&summary);
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let confirm_btn = egui::Button::new(
                                    egui::RichText::new("✔ 确认备份")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(46, 125, 50))
                                .rounding(egui::Rounding::same(6.0))
                                .min_size(egui::vec2(120.0, 34.0));

                                if ui.add(confirm_btn).clicked() {
                                    action = Some("confirm");
                                }

                                ui.add_space(10.0);

                                let cancel_btn = egui::Button::new(
                                    egui::RichText::new("✖ 取消")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(180, 50, 50))
                                .rounding(egui::Rounding::same(6.0))
                                .min_size(egui::vec2(100.0, 34.0));

                                if ui.add(cancel_btn).clicked() {
                                    action = Some("cancel");
                                }
                            },
                        );
                    });
                });

            match action {
                Some("confirm") => self.confirm_project_backup(),
                Some("cancel") => self.cancel_pending_backup(),
                _ => {}
            }
        }

        // 关闭前自动保存配置
        if ctx.input(|i| i.viewport().close_requested()) {
            let _ = save_config(&self.config);
        }
    }
}
