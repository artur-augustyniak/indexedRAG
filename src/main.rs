use std::path::{Path, PathBuf};

use eframe::{
    egui::{self, CentralPanel, Context, ScrollArea, SidePanel, TopBottomPanel, Ui},
    epi,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;

// =====================
// Data Structures
// =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,   // e.g. "user", "assistant", "system"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: i64,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub id: i64,
    pub root_paths: Vec<String>,
    pub index_interval_minutes: i32,
}

pub struct IndexedragApp {
    conn: Connection,
    conversation: Conversation,
    current_input: String,
    settings_open: bool,
    settings: AppSettings,
}

impl IndexedragApp {
    pub fn new() -> Self {
        let db_path = Self::get_db_path().join("indexedrag.db");
        std::fs::create_dir_all(db_path.parent().unwrap())
            .expect("Could not create config directory");

        let conn = Connection::open(&db_path).expect("Failed to open DB");
        Self::initialize_db(&conn);

        let conversation = Self::load_or_create_default_conversation(&conn);
        let settings = Self::load_or_create_default_settings(&conn);

        IndexedragApp {
            conn,
            conversation,
            current_input: String::new(),
            settings_open: false,
            settings,
        }
    }

    fn get_db_path() -> PathBuf {
        // Minimal cross-platform fallback approach
        if cfg!(target_os = "linux") {
            if let Some(home) = std::env::var_os("HOME") {
                PathBuf::from(home).join(".config").join("indexedrag")
            } else {
                PathBuf::from("indexedrag")
            }
        } else if cfg!(target_os = "windows") {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                PathBuf::from(appdata).join("indexedrag")
            } else {
                PathBuf::from("indexedrag")
            }
        } else if cfg!(target_os = "macos") {
            if let Some(home) = std::env::var_os("HOME") {
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("indexedrag")
            } else {
                PathBuf::from("indexedrag")
            }
        } else {
            PathBuf::from("indexedrag")
        }
    }

    fn initialize_db(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY,
                root_paths TEXT NOT NULL,
                index_interval_minutes INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversation (
                id INTEGER PRIMARY KEY,
                messages TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
    }

    fn load_or_create_default_conversation(conn: &Connection) -> Conversation {
        let mut stmt = conn
            .prepare("SELECT id, messages FROM conversation LIMIT 1")
            .expect("prepare conversation");
        let mut rows = stmt.query([]).expect("query conversation");

        if let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let messages_str: String = row.get(1).unwrap();
            let messages: Vec<Message> = serde_json::from_str(&messages_str).unwrap_or_default();
            Conversation { id, messages }
        } else {
            let default = Conversation {
                id: 1,
                messages: vec![Message {
                    role: "system".into(),
                    content: "Welcome to Indexedrag!".into(),
                }],
            };
            let messages_str = serde_json::to_string(&default.messages).unwrap();

            conn.execute(
                "INSERT INTO conversation (id, messages) VALUES (?1, ?2)",
                params![default.id, messages_str],
            )
            .unwrap();
            default
        }
    }

    fn load_or_create_default_settings(conn: &Connection) -> AppSettings {
        let mut stmt = conn
            .prepare("SELECT id, root_paths, index_interval_minutes FROM settings LIMIT 1")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();

        if let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let root_paths_str: String = row.get(1).unwrap();
            let root_paths = serde_json::from_str(&root_paths_str).unwrap_or(vec![]);
            let index_interval_minutes: i32 = row.get(2).unwrap();
            AppSettings {
                id,
                root_paths,
                index_interval_minutes,
            }
        } else {
            let default = AppSettings {
                id: 1,
                root_paths: vec!["/path/to/somewhere".into()],
                index_interval_minutes: 60,
            };
            let root_paths_str = serde_json::to_string(&default.root_paths).unwrap();
            conn.execute(
                "INSERT INTO settings (id, root_paths, index_interval_minutes)
                 VALUES (?1, ?2, ?3)",
                params![default.id, root_paths_str, default.index_interval_minutes],
            )
            .unwrap();
            default
        }
    }

    fn save_conversation(&self) {
        let messages_str = serde_json::to_string(&self.conversation.messages).unwrap();
        self.conn
            .execute(
                "UPDATE conversation SET messages = ?1 WHERE id = ?2",
                params![messages_str, self.conversation.id],
            )
            .unwrap();
    }

    fn save_settings(&self) {
        let root_paths_str = serde_json::to_string(&self.settings.root_paths).unwrap();
        self.conn
            .execute(
                "UPDATE settings
                 SET root_paths = ?1,
                     index_interval_minutes = ?2
                 WHERE id = ?3",
                params![
                    root_paths_str,
                    self.settings.index_interval_minutes,
                    self.settings.id
                ],
            )
            .unwrap();
    }

    fn call_llm_api_stub(&mut self, user_input: &str) {
        // In real use, this is where you'd talk to your LLM.
        let system_reply = format!("(Stub) LLM response to: '{}'", user_input);

        self.conversation.messages.push(Message {
            role: "assistant".into(),
            content: system_reply,
        });
    }
}

impl epi::App for IndexedragApp {
    fn name(&self) -> &str {
        "Indexedrag LLM Frontend"
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::epi::Frame) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("Settings").clicked() {
                    self.settings_open = !self.settings_open;
                }
            });
        });

        SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Conversations");
            ui.separator();
            ui.label("Placeholder for thread list, etc.");
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Indexedrag");
            ui.separator();
            self.draw_conversation_ui(ui);
        });

        if self.settings_open {
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    self.draw_settings_ui(ui);
                });
        }
    }
}

impl IndexedragApp {
    fn draw_conversation_ui(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            for msg in &self.conversation.messages {
                ui.group(|ui| {
                    ui.label(format!("{}: {}", msg.role, msg.content));
                });
                ui.separator();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Your message:");
            ui.text_edit_singleline(&mut self.current_input);
            if ui.button("Send").clicked() {
                let user_msg = Message {
                    role: "user".into(),
                    content: self.current_input.clone(),
                };
                self.conversation.messages.push(user_msg);

                self.call_llm_api_stub(&self.current_input);
                self.current_input.clear();
                self.save_conversation();
            }
        });
    }

    fn draw_settings_ui(&mut self, ui: &mut Ui) {
        ui.heading("Application Settings");
        ui.separator();

        ui.label("Indexed Root Paths:");
        let mut remove_indices = Vec::new();
        for (i, path) in self.settings.root_paths.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(path);
                if ui.button("Remove").clicked() {
                    remove_indices.push(i);
                }
            });
        }
        for i in remove_indices.into_iter().rev() {
            self.settings.root_paths.remove(i);
        }
        if ui.button("Add Another Path").clicked() {
            self.settings.root_paths.push(String::new());
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Index interval (minutes):");
            let mut interval_str = self.settings.index_interval_minutes.to_string();
            if ui.text_edit_singleline(&mut interval_str).lost_focus() {
                if let Ok(val) = interval_str.parse::<i32>() {
                    self.settings.index_interval_minutes = val;
                }
            }
        });

        ui.separator();
        if ui.button("Save Settings").clicked() {
            self.save_settings();
            self.settings_open = false;
        }
        ui.same_line();
        if ui.button("Cancel").clicked() {
            self.settings = Self::load_or_create_default_settings(&self.conn);
            self.settings_open = false;
        }
    }
}

fn main() {
    let app = IndexedragApp::new();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}
