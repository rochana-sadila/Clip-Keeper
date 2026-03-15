use arboard::Clipboard;
use chrono::Local;
use eframe::egui;
use rusqlite::{Connection, Result as SqlResult};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ─────────────────────────────────────────────
//  Database
// ─────────────────────────────────────────────

fn db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{}/.clipboard-manager.db", home)
}

fn open_db() -> SqlResult<Connection> {
    let conn = Connection::open(db_path())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clips (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            content   TEXT    NOT NULL,
            copied_at TEXT    NOT NULL
        );",
    )?;
    Ok(conn)
}

fn save_clip(conn: &Connection, content: &str) -> SqlResult<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO clips (content, copied_at) VALUES (?1, ?2)",
        rusqlite::params![content, now],
    )?;
    conn.execute(
        "DELETE FROM clips WHERE id NOT IN (
            SELECT id FROM clips ORDER BY id DESC LIMIT 200
        )",
        [],
    )?;
    Ok(())
}

fn load_clips(conn: &Connection) -> SqlResult<Vec<ClipEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, content, copied_at FROM clips ORDER BY id DESC LIMIT 200",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ClipEntry {
            id: row.get(0)?,
            content: row.get(1)?,
            copied_at: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn delete_clip(conn: &Connection, id: i64) -> SqlResult<()> {
    conn.execute("DELETE FROM clips WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}

fn clear_all_clips(conn: &Connection) -> SqlResult<()> {
    conn.execute("DELETE FROM clips", [])?;
    Ok(())
}

// ─────────────────────────────────────────────
//  Data types
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ClipEntry {
    id: i64,
    content: String,
    copied_at: String,
}

struct AppState {
    clips: Vec<ClipEntry>,
    db: Connection,
    search: String,
    status_msg: String,
}

impl AppState {
    fn new() -> Self {
        let db = open_db().expect("Cannot open database");
        let clips = load_clips(&db).unwrap_or_default();
        Self {
            clips,
            db,
            search: String::new(),
            status_msg: String::new(),
        }
    }

    fn reload(&mut self) {
        self.clips = load_clips(&self.db).unwrap_or_default();
    }
}

// ─────────────────────────────────────────────
//  Background clipboard watcher
// ─────────────────────────────────────────────

fn start_watcher(state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => { eprintln!("Clipboard error: {}", e); return; }
        };
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(500));
            let current = match clipboard.get_text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if current != last && !current.trim().is_empty() {
                last = current.clone();
                if let Ok(mut s) = state.lock() {
                    let _ = save_clip(&s.db, &current);
                    s.reload();
                }
            }
        }
    });
}

// ─────────────────────────────────────────────
//  GUI
// ─────────────────────────────────────────────

struct ClipboardApp {
    state: Arc<Mutex<AppState>>,
    clipboard: Option<Clipboard>,
}

impl ClipboardApp {
    fn new(state: Arc<Mutex<AppState>>) -> Self {
        Self { state, clipboard: Clipboard::new().ok() }
    }
}

impl eframe::App for ClipboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(Duration::from_millis(600));

        let (search_val_init, clips_snapshot, status) = {
            let s = self.state.lock().unwrap();
            (s.search.clone(), s.clips.clone(), s.status_msg.clone())
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::default()
                .fill(egui::Color32::from_rgb(28, 24, 24))
                .inner_margin(egui::Margin::same(12.0)))
            .show(ctx, |ui| {

                // ── Header ──
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📋 Clip Keeper").size(17.0)
                        .color(egui::Color32::from_rgb(220, 210, 200)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("🗑 Clear all")
                            .color(egui::Color32::from_rgb(200, 100, 80))).clicked()
                        {
                            if let Ok(mut s) = self.state.lock() {
                                let _ = clear_all_clips(&s.db);
                                s.reload();
                                s.status_msg = "Cleared all clips.".to_string();
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // ── Search ──
                let mut search_val = search_val_init;
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut search_val)
                        .hint_text("🔍  Search clips…")
                        .desired_width(f32::INFINITY),
                );
                if resp.changed() {
                    if let Ok(mut s) = self.state.lock() {
                        s.search = search_val.clone();
                    }
                }

                ui.add_space(4.0);

                // ── Status ──
                if !status.is_empty() {
                    ui.label(egui::RichText::new(&status).size(11.0)
                        .color(egui::Color32::from_rgb(100, 200, 120)));
                }

                ui.separator();

                // ── Clips list ──
                let filtered: Vec<&ClipEntry> = clips_snapshot.iter().filter(|c| {
                    search_val.is_empty()
                        || c.content.to_lowercase().contains(&search_val.to_lowercase())
                }).collect();

                if filtered.is_empty() {
                    ui.add_space(80.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Copy anything to add a clip")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(100, 90, 85)));
                    });
                } else {
                    let mut to_delete: Option<i64> = None;
                    let mut to_copy: Option<String> = None;

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for entry in &filtered {
                                let preview: String = entry.content.chars().take(120)
                                    .collect::<String>().replace('\n', " ");

                                egui::Frame::default()
                                    .fill(egui::Color32::from_rgb(42, 36, 34))
                                    .rounding(egui::Rounding::same(6.0))
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(egui::RichText::new(&preview).size(13.0)
                                                    .color(egui::Color32::from_rgb(210, 200, 190)));
                                                ui.label(egui::RichText::new(&entry.copied_at).size(10.0)
                                                    .color(egui::Color32::from_rgb(110, 100, 90)));
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.button(egui::RichText::new("🗑")
                                                        .color(egui::Color32::from_rgb(160, 80, 70)))
                                                        .on_hover_text("Delete").clicked()
                                                    {
                                                        to_delete = Some(entry.id);
                                                    }
                                                    if ui.button(egui::RichText::new("📋 Copy")
                                                        .color(egui::Color32::from_rgb(100, 180, 140)))
                                                        .clicked()
                                                    {
                                                        to_copy = Some(entry.content.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });

                    if let Some(text) = to_copy {
                        if let Some(cb) = &mut self.clipboard {
                            let _ = cb.set_text(text);
                            if let Ok(mut s) = self.state.lock() {
                                s.status_msg = "✓ Copied!".to_string();
                            }
                        }
                    }
                    if let Some(id) = to_delete {
                        if let Ok(mut s) = self.state.lock() {
                            let _ = delete_clip(&s.db, id);
                            s.reload();
                            s.status_msg = "Deleted.".to_string();
                        }
                    }
                }

                // ── Footer ──
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{} clips stored", clips_snapshot.len()))
                        .size(10.0)
                        .color(egui::Color32::from_rgb(80, 72, 68)));
                });
            });
    }
}

// ─────────────────────────────────────────────
//  Entry point
// ─────────────────────────────────────────────

fn main() {
    let state = Arc::new(Mutex::new(AppState::new()));
    start_watcher(Arc::clone(&state));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clip Keeper")
            .with_inner_size([380.0, 520.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Clip Keeper",
        options,
        Box::new(|_cc| Box::new(ClipboardApp::new(state))),
    )
    .expect("Failed to launch GUI");
}