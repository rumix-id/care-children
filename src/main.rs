use iced::widget::{button, column, container, row, scrollable, text, text_input, horizontal_rule, Space, svg, stack, image};
use iced::{Alignment, Element, Length, Color, Theme, Task, alignment};
use rusqlite::{params, Connection};

// ==========================================
// UI SETTINGS - EXACTLY FROM main.rs
// ==========================================
const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_BG: Color = Color::from_rgb(0.95, 0.95, 0.95);
const HOVER_COLOR: Color = Color::from_rgb(0.9, 0.9, 0.9);
const ACTIVE_TEXT: Color = Color::from_rgb(0.0, 0.58, 0.62);
const INACTIVE_TEXT: Color = Color::from_rgb(0.3, 0.3, 0.3);

const GAP_TITLE_TO_RULE: f32 = 8.0;      
const GAP_RULE_TO_CONTENT: f32 = 25.0;   

const ICON_SIZE: f32 = 24.0;
const ICON_SPACING: f32 = 15.0;
const BORDER_RADIUS: f32 = 6.0;
const MAIN_PADDING: u16 = 25;
const WIN_BTN_SIZE: u16 = 14;

const COLOR_GUARD_START: Color = Color::from_rgb(0.0, 0.58, 0.62);
const COLOR_GUARD_STOP: Color = Color::from_rgb(0.8, 0.2, 0.2);
const COLOR_ADD: Color = Color::from_rgb(0.2, 0.6, 0.2);
const COLOR_DELETE: Color = Color::from_rgb(0.8, 0.2, 0.2);
const COLOR_DISABLED: Color = Color::from_rgb(0.8, 0.8, 0.8);

pub fn main() -> iced::Result {
    let _ = init_database(); 
    iced::application("Guard Children's Internet", Guard::update, Guard::view)
        .window(iced::window::Settings {
            size: iced::Size::new(850.0, 550.0),
            resizable: false,
            position: iced::window::Position::Centered,
            decorations: false,
            ..Default::default()
        })
        .theme(|_| Theme::default()) 
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page { Overview, Setup, Security }

#[derive(Debug, Clone)]
struct BlacklistEntry { id: i32, domain: String, hits: i32 }

#[derive(Debug, Clone)]
enum Message {
    InputChanged(String), PasswordChanged(String), PasswordUnlockInput(String),
    AddDomain, StartGuard, StopGuard,
    ToggleEditPassword, SavePassword,
    ChangePage(Page), CloseWindow, MinimizeWindow,
    OpenEditPopup, CloseEditPopup, CancelEdit,
    ToggleSelectDomain(i32), DeleteSelected,
    CheckViolation(String), UnlockSystem,
}

struct Guard {
    password_admin: String,
    unlock_input: String,
    is_running: bool,
    is_editing_password: bool,
    is_hard_locked: bool,
    violation_count: u32,
    new_domain: String,
    blacklist: Vec<BlacklistEntry>,
    temp_blacklist: Vec<BlacklistEntry>, 
    current_page: Page,
    is_showing_edit_popup: bool,
    selected_for_delete: Vec<i32>,
    has_deleted: bool,
}

impl Default for Guard {
    fn default() -> Self {
        let (pass, list) = load_data_from_db();
        Self {
            password_admin: pass,
            unlock_input: String::new(),
            is_running: false,
            is_editing_password: false,
            is_hard_locked: false,
            violation_count: 0,
            new_domain: String::new(),
            blacklist: list,
            temp_blacklist: Vec::new(),
            current_page: Page::Overview,
            is_showing_edit_popup: false,
            selected_for_delete: Vec::new(),
            has_deleted: false,
        }
    }
}

impl Guard {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InputChanged(s) => self.new_domain = s,
            Message::PasswordChanged(s) => self.password_admin = s,
            Message::PasswordUnlockInput(s) => self.unlock_input = s,
            Message::AddDomain => {
                if !self.new_domain.is_empty() {
                    let domain = self.new_domain.clone();
                    if let Ok(id) = save_domain_to_db(&domain) {
                        self.blacklist.push(BlacklistEntry { id, domain, hits: 0 });
                        self.new_domain.clear();
                    }
                }
            }
            Message::StartGuard => self.is_running = true,
            Message::StopGuard => self.is_running = false,
            Message::ToggleEditPassword => self.is_editing_password = true,
            Message::SavePassword => {
                let _ = update_password_db(&self.password_admin);
                self.is_editing_password = false;
            }
            Message::ChangePage(p) => self.current_page = p,
            Message::CloseWindow => return iced::window::get_latest().and_then(iced::window::close),
            Message::MinimizeWindow => return iced::window::get_latest().and_then(|id| iced::window::minimize(id, true)),
            
            Message::OpenEditPopup => {
                self.is_showing_edit_popup = true;
                self.has_deleted = false;
                self.selected_for_delete.clear();
                self.temp_blacklist = self.blacklist.clone();
            }
            Message::CloseEditPopup => {
                // Hapus dari database saat klik Simpan
                for &id in &self.selected_for_delete { 
                    let _ = delete_domain_from_db(id); 
                }
                self.blacklist = self.temp_blacklist.clone();
                self.is_showing_edit_popup = false;
                self.has_deleted = false;
            }
            Message::CancelEdit => {
                self.is_showing_edit_popup = false;
                self.has_deleted = false;
            }
            Message::ToggleSelectDomain(id) => {
                if self.selected_for_delete.contains(&id) { self.selected_for_delete.retain(|&x| x != id); }
                else { self.selected_for_delete.push(id); }
            }
            Message::DeleteSelected => {
                self.temp_blacklist.retain(|e| !self.selected_for_delete.contains(&e.id));
                self.selected_for_delete.clear();
                self.has_deleted = true; 
            }
            Message::CheckViolation(domain) => {
                if self.is_running {
                    self.violation_count += 1;
                    let _ = increment_hits_db(&domain);
                    if self.violation_count >= 3 {
                        self.is_hard_locked = true;
                        return iced::window::get_latest().and_then(|id| iced::window::change_mode(id, iced::window::Mode::Fullscreen));
                    }
                }
            }
            Message::UnlockSystem => {
                if self.unlock_input == self.password_admin {
                    self.is_hard_locked = false;
                    self.violation_count = 0;
                    self.unlock_input.clear();
                    return iced::window::get_latest().and_then(|id| iced::window::change_mode(id, iced::window::Mode::Windowed));
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let window_controls = row![
            Space::with_width(Length::Fill),
            button(container(text("_").size(WIN_BTN_SIZE)).width(Length::Fill).center_x(Length::Fill))
                .on_press(Message::MinimizeWindow)
                .width(Length::Fixed(45.0)).padding(10)
                .style(|_, _| button::Style { 
                    background: Some(Color::from_rgb(0.9, 0.9, 0.9).into()), 
                    border: iced::border::Border { radius: 0.0.into(), ..Default::default() }, 
                    ..Default::default() 
                }),
            button(container(text("X").size(WIN_BTN_SIZE)).width(Length::Fill).center_x(Length::Fill))
                .on_press(Message::CloseWindow)
                .width(Length::Fixed(45.0)).padding(10)
                .style(|_, status| button::Style { 
                    background: Some(if status == button::Status::Hovered { Color::from_rgb(0.9, 0.1, 0.1).into() } else { Color::from_rgb(0.8, 0.2, 0.2).into() }), 
                    text_color: Color::WHITE, 
                    border: iced::border::Border { radius: 0.0.into(), ..Default::default() }, 
                    ..Default::default() 
                }),
        ].spacing(0);

        let sidebar = column![
            column![text("CARE CHILDREN").size(20).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }), text("v0.1.0").size(12).color([0.6, 0.6, 0.6])].spacing(5),
            Space::with_height(Length::Fixed(40.0)),
            self.menu_button("assets/home.svg", "Overview", Page::Overview),
            self.menu_button("assets/domain.svg", "Management", Page::Setup),
            self.menu_button("assets/settings.svg", "Security", Page::Security),
            Space::with_height(Length::Fill),
            text("© 2026 Rumix-id").size(10).color([0.7, 0.7, 0.7]),
        ].width(Length::Fixed(SIDEBAR_WIDTH)).spacing(10).padding(25);

        let main_view = match self.current_page {
            Page::Overview => self.view_overview(),
            Page::Setup => self.view_setup(),
            Page::Security => self.view_security(),
        };

        let base_ui = row![
            container(sidebar).style(|_| container::Style { background: Some(SIDEBAR_BG.into()), ..Default::default() }),
            column![window_controls, container(main_view).padding(MAIN_PADDING)].width(Length::Fill),
        ];

        if self.is_hard_locked {
            stack![
                base_ui,
                container(column![
                    text("SISTEM TERKUNCI").size(40).color(Color::WHITE).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
                    text("Batas percobaan terlampaui. Masukkan Password Admin:").color(Color::WHITE),
                    Space::with_height(20),
                    text_input("Password...", &self.unlock_input).on_input(Message::PasswordUnlockInput).padding(15).width(350),
                    button(text("Buka Kunci")).on_press(Message::UnlockSystem).padding(12).width(350).style(|_, _| button::Style { background: Some(COLOR_GUARD_START.into()), text_color: Color::WHITE, border: iced::border::Border { radius: 6.0.into(), ..Default::default() }, ..Default::default() })
                ].spacing(15).align_x(Alignment::Center))
                .width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill)
                .style(|_| container::Style { background: Some(Color { a: 0.95, r: 0.0, g: 0.0, b: 0.0 }.into()), ..Default::default() })
            ].into()
        } else {
            base_ui.into()
        }
    }

    fn menu_button<'a>(&self, icon_path: &'a str, label: &'a str, page: Page) -> Element<'a, Message> {
        let is_selected = self.current_page == page;
        button(row![
            svg(svg::Handle::from_path(icon_path))
                .width(Length::Fixed(ICON_SIZE))
                .height(Length::Fixed(ICON_SIZE))
                .style(move |_, _| svg::Style { color: Some(if is_selected { ACTIVE_TEXT } else { INACTIVE_TEXT }) }),
            text(label).size(14)
        ].spacing(ICON_SPACING).align_y(Alignment::Center))
            .on_press(Message::ChangePage(page))
            .width(Length::Fixed(180.0)).padding(12)
            .style(move |_, status| button::Style {
                background: if status == button::Status::Hovered || is_selected { Some(HOVER_COLOR.into()) } else { None },
                text_color: if is_selected { ACTIVE_TEXT } else { INACTIVE_TEXT },
                border: iced::border::Border { radius: BORDER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            }).into()
    }

    fn view_overview(&self) -> Element<'_, Message> {
        let status_title = if self.is_running { "Sistem Aktif" } else { "Perlindungan Mati" };
        let status_desc = if self.is_running { "Seluruh browser sedang dalam pengawasan." } else { "Sistem saat ini tidak mengawasi aktivitas." };
        let status_text_color = if self.is_running { Color::from_rgb(0.0, 1.0, 1.0) } else { Color::from_rgb(0.8, 0.2, 0.2) };

        column![
            text("Status Perlindungan").size(30),
            Space::with_height(Length::Fixed(GAP_TITLE_TO_RULE)),
            horizontal_rule(1),
            Space::with_height(Length::Fixed(GAP_RULE_TO_CONTENT)),
            stack![
                image(if self.is_running { "assets/1.png" } else { "assets/2.png" })
                    .width(Length::Fixed(600.0))
                    .height(Length::Fixed(130.0))
                    .content_fit(iced::ContentFit::Cover),
                container(row![
                    svg(svg::Handle::from_path(if self.is_running { "assets/on.svg" } else { "assets/off.svg" }))
                        .width(Length::Fixed(60.0)).height(Length::Fixed(60.0)),
                    column![
                        text(status_title).size(22).color(status_text_color).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
                        text(status_desc).size(14).color(if self.is_running { Color::from_rgb(0.9, 0.9, 0.9) } else { Color::from_rgb(0.7, 0.7, 0.7) }),
                    ].spacing(5)
                ].spacing(20).align_y(Alignment::Center)).padding(25).center_y(Length::Fill)
            ],
            Space::with_height(Length::Fixed(30.0)),
            row![
                button(container(text("Mulai Guard").size(16)).width(Length::Fill).center_x(Length::Fill))
                    .on_press_maybe(if !self.is_running { Some(Message::StartGuard) } else { None })
                    .width(Length::Fixed(160.0)).padding(12)
                    .style(move |_, _| button::Style { 
                        background: Some((if self.is_running { COLOR_DISABLED } else { COLOR_GUARD_START }).into()), 
                        text_color: Color::WHITE, 
                        border: iced::border::Border { radius: 6.0.into(), ..Default::default() }, 
                        ..Default::default() 
                    }),
                button(container(text("Hentikan Guard").size(16)).width(Length::Fill).center_x(Length::Fill))
                    .on_press_maybe(if self.is_running { Some(Message::StopGuard) } else { None })
                    .width(Length::Fixed(160.0)).padding(12)
                    .style(move |_, _| button::Style { 
                        background: Some((if self.is_running { COLOR_GUARD_STOP } else { COLOR_DISABLED }).into()), 
                        text_color: Color::WHITE, 
                        border: iced::border::Border { radius: 6.0.into(), ..Default::default() }, 
                        ..Default::default() 
                    }),
            ].spacing(15),
            Space::with_height(Length::Fixed(151.0)),
            text("Program berjalan dilatar belakang, untuk kembali membuka tekan SHIFT+F1+DELETE")
                .size(12).color([0.4, 0.4, 0.4]).width(Length::Fill),
        ].into()
    }

    fn view_setup(&self) -> Element<'_, Message> {
        let table_header = container(row![
            text("Domain / IP").width(Length::Fill).align_x(alignment::Horizontal::Left),
            text("Hits").width(80).align_x(alignment::Horizontal::Center),
        ].padding(12).align_y(Alignment::Center))
        .style(|_| container::Style { 
            background: Some(Color::from_rgb(0.92, 0.92, 0.92).into()),
            border: iced::border::Border { color: Color::from_rgb(0.7, 0.7, 0.7), width: 1.0, ..Default::default() },
            ..Default::default() 
        });

        let main_setup_content = column![
            text("Setup Daftar Blokir").size(30),
            Space::with_height(Length::Fixed(GAP_TITLE_TO_RULE)),
            horizontal_rule(1),
            Space::with_height(Length::Fixed(GAP_RULE_TO_CONTENT)),
            row![
                text_input("Tambah domain baru...", &self.new_domain).on_input(Message::InputChanged).padding(10),
                button(text("Tambah").size(14)).on_press(Message::AddDomain).padding(10)
                    .style(move |_, _| button::Style { 
                        background: Some(COLOR_ADD.into()), 
                        text_color: Color::WHITE, 
                        border: iced::border::Border { radius: BORDER_RADIUS.into(), ..Default::default() }, 
                        ..Default::default() 
                    }),
            ].spacing(10),
            Space::with_height(Length::Fixed(15.0)),
            column![
                table_header,
                scrollable(column(self.blacklist.iter().enumerate().map(|(i, e)| {
                    let row_bg = if i % 2 == 0 { Color::from_rgb(0.96, 0.97, 0.98) } else { Color::WHITE };
                    container(row![
                        text(&e.domain).width(Length::Fill).align_x(alignment::Horizontal::Left),
                        text(format!("{}", e.hits)).width(80).align_x(alignment::Horizontal::Center).color([0.5, 0.5, 0.5]),
                    ].align_y(Alignment::Center).padding(10))
                    .style(move |_| container::Style { background: Some(row_bg.into()), ..Default::default() }).into()
                }).collect::<Vec<_>>()))
                .height(Length::Fixed(200.0))
                .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(0).margin(0))),
                Space::with_height(Length::Fixed(45.0)),
                row![
                    text("Pilih 'Edit' untuk mengelola, mengubah, atau menghapus daftar domain yang ada.").size(12).color([0.4, 0.4, 0.4]).width(Length::Fill),
                    button(text("Edit")).on_press(Message::OpenEditPopup).padding(8)
                        .style(move |_, _| button::Style { background: Some(Color::from_rgb(0.9, 0.9, 0.9).into()), border: iced::border::Border { radius: 5.0.into(), ..Default::default() }, ..Default::default() }),
                ].spacing(20).align_y(Alignment::Center)
            ]
        ];

        if self.is_showing_edit_popup {
            stack![
                main_setup_content,
                container(Space::with_width(Length::Fill).height(Length::Fill)).style(|_| container::Style { background: Some(Color { a: 0.4, ..Color::BLACK }.into()), ..Default::default() }),
                container(self.view_edit_modal()).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill)
            ].into()
        } else {
            main_setup_content.into()
        }
    }

    fn view_edit_modal(&self) -> Element<'_, Message> {
        let is_anything_selected = !self.selected_for_delete.is_empty();
        let can_save = self.has_deleted;

        let modal_content = column![
            text("Kelola Daftar Blokir").size(20).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
            Space::with_height(Length::Fixed(15.0)),
            scrollable(column(self.temp_blacklist.iter().enumerate().map(|(i, e)| {
                let is_selected = self.selected_for_delete.contains(&e.id);
                let row_bg = if i % 2 != 0 { Color::from_rgb(0.97, 0.97, 0.98) } else { Color::WHITE };
                container(button(row![
                    text(if is_selected { "●" } else { "○" }).size(14).color(if is_selected { COLOR_DELETE } else { INACTIVE_TEXT }),
                    text(&e.domain).width(Length::Fill),
                ].spacing(12).align_y(Alignment::Center)).on_press(Message::ToggleSelectDomain(e.id)).padding(10).width(Length::Fill)
                .style(move |_, status| button::Style {
                    background: if is_selected { Some(HOVER_COLOR.into()) } else if status == button::Status::Hovered { Some(Color::from_rgb(0.93, 0.93, 0.93).into()) } else { None },
                    ..Default::default()
                }))
                .style(move |_| container::Style { background: Some(row_bg.into()), border: iced::border::Border { color: Color::from_rgb(0.9, 0.9, 0.9), width: 0.5, ..Default::default() }, ..Default::default() }).into()
            }).collect::<Vec<_>>())).height(Length::Fixed(250.0)),
            Space::with_height(Length::Fixed(20.0)),
            row![
                button(text("Hapus Terpilih")).on_press_maybe(if is_anything_selected { Some(Message::DeleteSelected) } else { None }).padding(10)
                .style(move |_, _| button::Style { 
                    background: Some((if is_anything_selected { COLOR_DELETE } else { COLOR_DISABLED }).into()), 
                    text_color: Color::WHITE, border: iced::border::Border { radius: 5.0.into(), ..Default::default() }, ..Default::default() 
                }),
                Space::with_width(Length::Fill),
                button(text("Simpan")).on_press_maybe(if can_save { Some(Message::CloseEditPopup) } else { None }).padding(10)
                .style(move |_, _| button::Style { 
                    background: Some((if can_save { COLOR_GUARD_START } else { COLOR_DISABLED }).into()), 
                    text_color: Color::WHITE, border: iced::border::Border { radius: 5.0.into(), ..Default::default() }, ..Default::default() 
                }),
                button(text("Cancel")).on_press(Message::CancelEdit).padding(10).style(button::secondary),
            ].spacing(10)
        ].padding(25).width(Length::Fixed(450.0));

        container(modal_content).style(|_| container::Style { 
            background: Some(Color::WHITE.into()),
            border: iced::border::Border { radius: 12.0.into(), width: 1.0, color: Color::from_rgb(0.8, 0.8, 0.8), ..Default::default() },
            shadow: iced::Shadow { color: Color { a: 0.2, ..Color::BLACK }, offset: iced::Vector::new(0.0, 4.0), blur_radius: 10.0 },
            ..Default::default() 
        }).into()
    }

    fn view_security(&self) -> Element<'_, Message> {
        let (btn_text, btn_msg, btn_color, txt_color) = if self.is_editing_password {
            ("Simpan Password", Message::SavePassword, ACTIVE_TEXT, Color::WHITE)
        } else {
            ("Update Password", Message::ToggleEditPassword, Color::from_rgb(0.85, 0.85, 0.85), Color::BLACK)
        };

        column![
            text("Keamanan").size(30),
            Space::with_height(Length::Fixed(GAP_TITLE_TO_RULE)),
            horizontal_rule(1),
            Space::with_height(Length::Fixed(GAP_RULE_TO_CONTENT)),
            text("Password Admin:").size(16),
            text(if self.is_editing_password { "Sedang dalam mode edit. Tekan Simpan untuk memperbarui." } else { "Password saat ini terkunci. Klik Update untuk mengubah." })
                .size(12).color(if self.is_editing_password { ACTIVE_TEXT } else { Color::from_rgb(0.5, 0.5, 0.5) }),
            Space::with_height(Length::Fixed(10.0)), 
            row![
                text_input("Password admin...", &self.password_admin).on_input_maybe(if self.is_editing_password { Some(Message::PasswordChanged) } else { None }).padding(10).width(300),
                button(text(btn_text).size(14)).on_press(btn_msg).padding(12) 
                    .style(move |_, _| button::Style {
                        background: Some(btn_color.into()), text_color: txt_color,
                        border: iced::border::Border { radius: 6.0.into(), ..Default::default() }, ..Default::default()
                    }),
            ].spacing(15).align_y(Alignment::Center),
        ].spacing(5).into()
    }
}

// --- DATABASE LOGIC (CITED FROM 1main.rs) ---
fn init_database() -> rusqlite::Result<()> {
    let conn = Connection::open("guard_data.db")?;
    conn.execute("CREATE TABLE IF NOT EXISTS blacklist (id INTEGER PRIMARY KEY, domain TEXT NOT NULL, hits INTEGER DEFAULT 0)", [])?;
    conn.execute("CREATE TABLE IF NOT EXISTS config (key TEXT PRIMARY KEY, value TEXT NOT NULL)", [])?;
    conn.execute("INSERT OR IGNORE INTO config (key, value) VALUES ('password', 'admin123')", [])?;
    Ok(())
}

fn load_data_from_db() -> (String, Vec<BlacklistEntry>) {
    let conn = Connection::open("guard_data.db").unwrap();
    let pass: String = conn.query_row("SELECT value FROM config WHERE key = 'password'", [], |r| r.get(0)).unwrap_or("admin123".to_string());
    let mut stmt = conn.prepare("SELECT id, domain, hits FROM blacklist").unwrap();
    let list = stmt.query_map([], |row| {
        Ok(BlacklistEntry { id: row.get::<_, i32>(0).unwrap(), domain: row.get::<_, String>(1).unwrap(), hits: row.get::<_, i32>(2).unwrap() })
    }).unwrap().filter_map(|e| e.ok()).collect();
    (pass, list)
}

fn save_domain_to_db(domain: &str) -> rusqlite::Result<i32> {
    let conn = Connection::open("guard_data.db")?;
    conn.execute("INSERT INTO blacklist (domain, hits) VALUES (?, 0)", params![domain])?;
    Ok(conn.last_insert_rowid() as i32)
}

fn delete_domain_from_db(id: i32) -> rusqlite::Result<()> {
    let conn = Connection::open("guard_data.db")?;
    conn.execute("DELETE FROM blacklist WHERE id = ?", params![id])?;
    Ok(())
}

fn update_password_db(pass: &str) -> rusqlite::Result<()> {
    let conn = Connection::open("guard_data.db")?;
    conn.execute("UPDATE config SET value = ? WHERE key = 'password'", params![pass])?;
    Ok(())
}

fn increment_hits_db(domain: &str) -> rusqlite::Result<()> {
    let conn = Connection::open("guard_data.db")?;
    conn.execute("UPDATE blacklist SET hits = hits + 1 WHERE domain = ?", params![domain])?;
    Ok(())
}