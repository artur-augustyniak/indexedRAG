use std::path::PathBuf;

use directories::ProjectDirs;
use eframe::{
    egui::{self, CentralPanel, Context, ScrollArea, SidePanel, TopBottomPanel, Ui},
    App, Frame, NativeOptions,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // e.g. "user", "assistant", "system"
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
        let db_path = Self::get_db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).expect("Could not create config directory");
        }
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

    /// Return a platform-appropriate path to the database file:
    ///  - Linux:   ~/.config/indexedrag/indexedrag.db
    ///  - Windows: %APPDATA%\indexedrag\indexedrag.db
    ///  - macOS:   ~/Library/Application Support/indexedrag/indexedrag.db
    fn get_db_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("pl", "aaugustyniak", "indexedRAG") {
            let config_dir = proj_dirs.config_dir();
            config_dir.join("indexedRAG.db")
        } else {
            PathBuf::from("indexedRAG.db")
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
        .expect("Failed to create settings table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversation (
                id INTEGER PRIMARY KEY,
                messages TEXT NOT NULL
            )",
            [],
        )
        .expect("Failed to create conversation table");
    }

    fn load_or_create_default_conversation(conn: &Connection) -> Conversation {
        let mut stmt = conn
            .prepare("SELECT id, messages FROM conversation LIMIT 1")
            .expect("Failed to prepare conversation select");
        let mut rows = stmt.query([]).expect("Failed to query conversation table");

        if let Some(row) = rows.next().expect("Failed to iterate conversation rows") {
            let id: i64 = row.get(0).expect("Failed to get conversation id");
            let messages_str: String = row.get(1).expect("Failed to get conversation messages");
            let messages: Vec<Message> =
                serde_json::from_str(&messages_str).unwrap_or_else(|_| vec![]);

            Conversation { id, messages }
        } else {
            let default = Conversation {
                id: 1,
                messages: vec![Message {
                    role: "system".into(),
                    content: "Welcome to Indexedrag!".into(),
                }],
            };
            let messages_str = serde_json::to_string(&default.messages).expect("Serialize fail");

            conn.execute(
                "INSERT INTO conversation (id, messages) VALUES (?1, ?2)",
                params![default.id, messages_str],
            )
            .expect("Failed to insert default conversation");

            default
        }
    }

    fn load_or_create_default_settings(conn: &Connection) -> AppSettings {
        let mut stmt = conn
            .prepare("SELECT id, root_paths, index_interval_minutes FROM settings LIMIT 1")
            .expect("Failed to prepare settings select");
        let mut rows = stmt.query([]).expect("Failed to query settings table");

        if let Some(row) = rows.next().expect("Failed to iterate settings rows") {
            let id: i64 = row.get(0).expect("Failed to get settings id");
            let root_paths_str: String = row.get(1).expect("Failed to get root_paths");
            let root_paths: Vec<String> =
                serde_json::from_str(&root_paths_str).unwrap_or_else(|_| vec![]);
            let index_interval_minutes: i32 = row.get(2).expect("Failed to get index_interval");

            AppSettings {
                id,
                root_paths,
                index_interval_minutes,
            }
        } else {
            let default = AppSettings {
                id: 1,
                root_paths: vec!["/path/to/somewhere".to_string()],
                index_interval_minutes: 60,
            };

            let root_paths_str =
                serde_json::to_string(&default.root_paths).expect("Failed to serialize root paths");
            conn.execute(
                "INSERT INTO settings (id, root_paths, index_interval_minutes)
                 VALUES (?1, ?2, ?3)",
                params![default.id, root_paths_str, default.index_interval_minutes],
            )
            .expect("Failed to insert default settings");

            default
        }
    }

    fn save_conversation(&self) {
        let messages_str = serde_json::to_string(&self.conversation.messages)
            .expect("Failed to serialize messages");
        self.conn
            .execute(
                "UPDATE conversation SET messages = ?1 WHERE id = ?2",
                params![messages_str, self.conversation.id],
            )
            .expect("Failed to update conversation");
    }

    fn save_settings(&self) {
        let root_paths_str = serde_json::to_string(&self.settings.root_paths)
            .expect("Failed to serialize root paths");
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
            .expect("Failed to update settings");
    }

    /// (Stub) This would call external LLM APIs in JSON format. Currently just simulates a response.
    fn call_llm_api_stub(&mut self, user_input: &str) {
        // In a real app, you would send the conversation history plus the new user message
        // to an LLM endpoint, e.g. OpenAI, llama.cpp, etc., in JSON format.
        // For now, just simulate a response:
        let system_reply = format!("(Stub) LLM Response to: '{}'", user_input);

        // Add the assistant message
        self.conversation.messages.push(Message {
            role: "assistant".into(),
            content: system_reply,
        });
    }

    fn draw_conversation_ui(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            // .auto_shrink([false; 2])
            .show(ui, |ui| {
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
                    role: "user".to_string(),
                    content: self.current_input.clone(),
                };
                self.conversation.messages.push(user_msg);
                let input_clone = self.current_input.clone();
                self.call_llm_api_stub(&input_clone);
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

        for i in remove_indices.iter().rev() {
            self.settings.root_paths.remove(*i);
        }

        if ui.button("Add Another Path").clicked() {
            self.settings.root_paths.push("".to_string());
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

        ui.horizontal(|ui| {
            if ui.button("Save Settings").clicked() {
                self.save_settings();
                self.settings_open = false;
            }

            if ui.button("Cancel").clicked() {
                self.settings = Self::load_or_create_default_settings(&self.conn);
                self.settings_open = false;
            }
        });
    }
}

// =====================
// Implement eframe::App
// =====================
impl App for IndexedragApp {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        // You can set a window title dynamically if you want:
        // frame.set_window_title("Indexedrag LLM Frontend");
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
            ui.label("Placeholder for threads list, etc.");
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

fn main() {
    let app = IndexedragApp::new();
    let mut native_options = NativeOptions::default();
    native_options.initial_window_size = Some(egui::vec2(1000.0, 800.0));

    eframe::run_native(
        // window title:
        "indexedRAG",
        native_options,
        Box::new(|_cc| Box::new(app)),
    );
}
