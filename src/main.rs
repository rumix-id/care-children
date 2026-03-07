use iced::widget::{button, column, container, row, scrollable, text, text_input, horizontal_rule, Space, svg, stack, image};
use iced::{Alignment, Element, Length, Color, Theme, Task, alignment};
use rusqlite::{params, Connection};
use std::fs::{self, OpenOptions}; // Menambahkan fs di sini
use std::io::Write;
use std::time::Duration;

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
        .subscription(Guard::subscription) // WAJIB ADA INI
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page { Overview, Setup, Security }

#[derive(Debug, Clone)]
struct BlacklistEntry { id: i32, domain: String, hits: i32 }

#[derive(Debug, Clone)]
enum Message {
    // --- Navigasi ---
    SetPage(Page),              // Sesuaikan dengan UI yang memanggil SetPage
    ChangePage(Page),           // Jika sidebar pakai ini
    
    // --- Input ---
    InputChanged(String),
    PasswordChanged(String),    
    PasswordUnlockInput(String), // Ini yang diminta oleh error di baris 377
    
    // --- Window ---
    CloseWindow, 
    MinimizeWindow,

    // --- Manajemen Domain ---
    AddDomain,
    OpenEditPopup,
    CloseEditPopup,
    CancelEdit,
    ToggleSelectForDelete(i32), 
    DeleteSelected,

    // --- Guard ---
    StartGuard,
    StopGuard,
    ToggleEditPassword,
    SavePassword,
    CheckViolation(String),
    UnlockSystem,
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
// 1. Agar tulisan di kotak input berubah saat diketik
        Message::PasswordChanged(new_val) => {
            self.password_admin = new_val; // Menghubungkan ketikan ke variabel password
        }

        // 2. Agar status edit berubah (tombol Update ke Simpan)
        Message::ToggleEditPassword => {
            self.is_editing_password = !self.is_editing_password;
        }

        // 3. Agar password tersimpan permanen ke Database
        Message::SavePassword => {
            if !self.password_admin.is_empty() {
                // Simpan password baru ke database guard_data.db
                let _ = update_password_db(&self.password_admin);
                
                // Matikan mode edit agar input terkunci kembali
                self.is_editing_password = false;
                
                println!("Password admin telah diperbarui.");
            }
        }
        // --- NAVIGASI SIDEBAR ---
        Message::ChangePage(page) | Message::SetPage(page) => {
            self.current_page = page; // Pastikan menggunakan current_page
        }

        // --- KONTROL JENDELA ---
        Message::MinimizeWindow => {
            return iced::window::get_latest().and_then(|id| {
                iced::window::minimize(id, true)
            });
        }
        Message::CloseWindow => {
            return iced::window::get_latest().and_then(iced::window::close);
        }

        // --- MANAJEMEN DOMAIN ---
        Message::InputChanged(val) => self.new_domain = val,
        
        Message::AddDomain => {
            if !self.new_domain.is_empty() {
                let cleaned = self.new_domain
                    .replace("https://", "").replace("http://", "").replace("www.", "")
                    .split('/').next().unwrap_or("").trim().to_string();

                if !cleaned.is_empty() {
                    if let Ok(id) = save_domain_to_db(&cleaned) {
                        self.blacklist.push(BlacklistEntry { id, domain: cleaned.clone(), hits: 0 });
                        self.new_domain.clear();
                        if self.is_running { 
                            let _ = self.apply_hosts_block(); 
                        }
                    }
                }
            }
        }

        // --- MODAL & DELETE ---
        Message::OpenEditPopup => {
            self.temp_blacklist = self.blacklist.clone();
            self.selected_for_delete.clear();
            self.has_deleted = false;
            self.is_showing_edit_popup = true;
        }

        Message::ToggleSelectForDelete(id) => {
            if let Some(pos) = self.selected_for_delete.iter().position(|&x| x == id) {
                self.selected_for_delete.remove(pos);
            } else {
                self.selected_for_delete.push(id);
            }
        }

        Message::DeleteSelected => {
            self.temp_blacklist.retain(|e| !self.selected_for_delete.contains(&e.id));
            self.has_deleted = true;
        }

        Message::CloseEditPopup => {
            for &id in &self.selected_for_delete {
                let _ = delete_domain_from_db(id);
            }
            self.blacklist = self.temp_blacklist.clone();
            self.is_showing_edit_popup = false;
            self.selected_for_delete.clear();
            self.has_deleted = false;

            if self.is_running { let _ = self.apply_hosts_block(); }
            else { let _ = self.clear_hosts_block(); }
        }

        Message::CancelEdit => {
            self.is_showing_edit_popup = false;
            self.selected_for_delete.clear();
            self.has_deleted = false;
        }

        // --- GUARD CONTROL ---
        Message::StartGuard => {
            self.is_running = true;
            let _ = self.apply_hosts_block();
        }

        Message::StopGuard => {
            self.is_running = false;
            let _ = self.clear_hosts_block();
        }

        // --- KEAMANAN ---
        Message::CheckViolation(info) => {
            // Gunakan info agar warning hilang
            if self.is_running && !self.is_hard_locked {
                let keywords: Vec<String> = self.blacklist.iter()
                    .map(|e| e.domain.to_lowercase().replace("www.", "").replace(".com", ""))
                    .collect();

                if let Some(detected) = self.check_window_titles(keywords) {
                    println!("Pelanggaran: {} | Status: {}", detected, info);
                    let _ = increment_hits_db(&detected); 
                    self.violation_count += 1;
                    self.is_hard_locked = true;
                    return iced::window::get_latest().and_then(|id| {
                        iced::window::change_mode(id, iced::window::Mode::Fullscreen)
                    });
                }
            }
        }

        Message::PasswordUnlockInput(val) => self.unlock_input = val,
        
        Message::UnlockSystem => {
            if self.unlock_input == self.password_admin {
                self.is_hard_locked = false;
                self.unlock_input.clear();
                return iced::window::get_latest().and_then(|id| {
                    iced::window::change_mode(id, iced::window::Mode::Windowed)
                });
            } else {
                self.unlock_input.clear();
            }
        }
        
        _ => {}
    }
    Task::none()
}

pub fn subscription(&self) -> iced::Subscription<Message> {
    if self.is_running {
        iced::time::every(std::time::Duration::from_millis(1000))
            .map(|_| Message::CheckViolation("Watchdog Aktif".to_string()))
    } else {
        iced::Subscription::none()
    }
}
fn check_window_titles(&self, keywords: Vec<String>) -> Option<String> {
    use std::process::Command;

    // Ambil semua judul jendela yang aktif
    let output = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            "Get-Process | Where-Object {$_.MainWindowTitle -ne ''} | Select-Object -ExpandProperty MainWindowTitle"
        ])
        .output();

    if let Ok(out) = output {
        let titles = String::from_utf8_lossy(&out.stdout).to_lowercase();
        
        for kw in keywords {
            // Jika judul jendela mengandung kata kunci (misal "youtube")
            if !kw.is_empty() && titles.contains(&kw) {
                // Kill SEMUA browser secara paksa
                let _ = Command::new("taskkill").args(&["/F", "/IM", "chrome.exe"]).spawn();
                let _ = Command::new("taskkill").args(&["/F", "/IM", "msedge.exe"]).spawn();
                let _ = Command::new("taskkill").args(&["/F", "/IM", "firefox.exe"]).spawn();
                return Some(kw);
            }
        }
    }
    None
}

fn apply_hosts_block(&self) -> std::io::Result<()> {
    self.clear_hosts_block()?; // Bersihkan dulu agar tidak duplikat
    let hosts_path = "C:\\Windows\\System32\\drivers\\etc\\hosts";
    
    let mut file = OpenOptions::new().append(true).open(hosts_path)?;
    writeln!(file, "\n# --- GUARD CHILDREN START ---")?;
    
    for entry in &self.blacklist {
        writeln!(file, "127.0.0.1 {}", entry.domain)?;
        writeln!(file, "::1 {}", entry.domain)?;
        writeln!(file, "127.0.0.1 www.{}", entry.domain)?;
        writeln!(file, "::1 www.{}", entry.domain)?;
    }
    
    writeln!(file, "# --- GUARD CHILDREN END ---")?;
    Ok(())
}

fn clear_hosts_block(&self) -> std::io::Result<()> {
    let hosts_path = "C:\\Windows\\System32\\drivers\\etc\\hosts";
    
    // Membaca file, jika tidak ada/error kita abaikan
    let content = fs::read_to_string(hosts_path).unwrap_or_default();
    
    // Filter baris secara eksplisit untuk menghapus jejak program
    let clean_lines: Vec<String> = content
        .lines()
        .filter(|line: &&str| { // Memberikan tipe eksplisit agar tidak error E0282
            !line.contains("# --- GUARD CHILDREN") && 
            !line.contains("127.0.0.1") && 
            !line.contains("::1") || 
            // Tetap simpan baris localhost bawaan Windows
            line.trim() == "127.0.0.1 localhost" || line.trim() == "::1 localhost"
        })
        .map(|s| s.to_string())
        .collect();

    let mut file = OpenOptions::new().write(true).truncate(true).open(hosts_path)?;
    for line in clean_lines {
        if !line.trim().is_empty() {
            writeln!(file, "{}", line)?;
        }
    }
    Ok(())
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

    // 1. KONTEN MODAL (Struktur kolom utama dari main.rs)
    let modal_content = column![
        // Judul Modal (Tanpa .bold() sesuai permintaan Anda)
        text("Manajemen Daftar Blokir").size(20),
        Space::with_height(Length::Fixed(15.0)),
        
        // 2. AREA DAFTAR (Menggunakan scrollable agar tidak merusak layout)
        scrollable(column(
            self.temp_blacklist.iter().enumerate().map(|(i, e)| {
                let is_selected = self.selected_for_delete.contains(&e.id);
                let row_bg = if i % 2 != 0 { Color::from_rgb(0.97, 0.97, 0.98) } else { Color::WHITE };

                container(
                    button(row![
                        text(if is_selected { "●" } else { "○" }).size(14).color(if is_selected { COLOR_DELETE } else { INACTIVE_TEXT }),
                        text(&e.domain).width(Length::Fill),
                    ].spacing(12).align_y(Alignment::Center))
                    .on_press(Message::ToggleSelectForDelete(e.id)) // Menggunakan ToggleSelectForDelete
                    .padding(10)
                    .style(move |_, status| button::Style {
                        background: if is_selected { Some(HOVER_COLOR.into()) } 
                                   else if status == button::Status::Hovered { Some(Color::from_rgb(0.93, 0.93, 0.93).into()) } 
                                   else { None },
                        ..Default::default()
                    })
                )
                .style(move |_| container::Style { 
                    background: Some(row_bg.into()), 
                    border: iced::border::Border { color: Color::from_rgb(0.9, 0.9, 0.9), width: 0.5, ..Default::default() },
                    ..Default::default() 
                })
                .into()
            }).collect::<Vec<_>>()
        )).height(Length::Fixed(250.0)),

        Space::with_height(Length::Fixed(20.0)),

        // 3. BARIS TOMBOL (Footer)
        row![
            // Tombol Hapus Terpilih
            button(text("Hapus Terpilih").size(14))
                .on_press_maybe(if !self.has_deleted && is_anything_selected { Some(Message::DeleteSelected) } else { None })
                .padding(10)
                .style(move |_, _| {
                    let bg = if !self.has_deleted && is_anything_selected { COLOR_DELETE } else { COLOR_DISABLED };
                    button::Style { 
                        background: Some(bg.into()), 
                        text_color: Color::WHITE, 
                        border: iced::border::Border { radius: 5.0.into(), ..Default::default() },
                        ..Default::default()
                    }
                }),

            Space::with_width(Length::Fill),

            // Tombol Simpan
            button(text("Simpan").size(14))
                .on_press_maybe(if can_save { Some(Message::CloseEditPopup) } else { None })
                .padding(10)
                .style(move |_, _| {
                    let bg = if can_save { COLOR_GUARD_START } else { COLOR_DISABLED };
                    button::Style { 
                        background: Some(bg.into()), 
                        text_color: Color::WHITE, 
                        border: iced::border::Border { radius: 5.0.into(), ..Default::default() },
                        ..Default::default()
                    }
                }),

            // Tombol Cancel
            button(text("Cancel").size(14))
                .on_press(Message::CancelEdit)
                .padding(10)
                .style(|_, _| button::Style {
                    background: Some(Color::from_rgb(0.6, 0.6, 0.6).into()),
                    text_color: Color::WHITE,
                    border: iced::border::Border { radius: 5.0.into(), ..Default::default() },
                    ..Default::default()
                }),
        ].spacing(10)
    ]
    .padding(25)
    .width(Length::Fixed(500.0)); // Diperlebar sedikit dari main.rs agar tidak sesak

    // 4. WRAPPER CONTAINER (Sesuai dengan style main.rs)
    container(modal_content)
        .style(|_| container::Style { 
            background: Some(Color::WHITE.into()),
            border: iced::border::Border { 
                radius: 12.0.into(), 
                width: 1.0, 
                color: Color::from_rgb(0.8, 0.8, 0.8), 
                ..Default::default() 
            },
            shadow: iced::Shadow { 
                color: Color { a: 0.2, ..Color::BLACK }, 
                offset: iced::Vector::new(0.0, 4.0), 
                blur_radius: 10.0 
            },
            ..Default::default() 
        })
        .into()
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