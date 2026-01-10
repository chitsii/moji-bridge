use iced::keyboard::{self, Key};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{event, Element, Event, Font, Length, Size, Subscription, Task};
use iced::{Background, Border, Color, Theme};
use iced::window;
// time module not available in iced 0.14 without feature
use std::sync::OnceLock;

use crate::clipboard_utils;
use crate::hook;
use crate::terminal;
use crate::logger;

/// Configuration for resident mode (stored globally using OnceLock for thread safety)
static RESIDENT_CONFIG: OnceLock<ResidentConfigData> = OnceLock::new();

#[derive(Clone)]
struct ResidentConfigData {
    #[allow(dead_code)]
    session_id: String,
    #[allow(dead_code)]
    cwd: String,
    label: Option<String>,
    terminal_hwnd: Option<isize>,
}

/// Configuration for resident mode
pub struct ResidentConfig {
    pub session_id: String,
    pub cwd: String,
    pub label: Option<String>,
    pub terminal_hwnd: Option<isize>,
}

/// The main application state for resident mode
pub struct ResidentClaudeInput {
    content: text_editor::Content,
    status_message: Option<String>,
}

impl Default for ResidentClaudeInput {
    fn default() -> Self {
        Self {
            content: text_editor::Content::new(),
            status_message: None,
        }
    }
}

fn resident_theme(_state: &ResidentClaudeInput) -> Theme {
    Theme::Dark
}

/// Messages for the resident application
#[derive(Debug, Clone)]
pub enum ResidentMessage {
    EditorAction(text_editor::Action),
    Submit,        // Send via direct paste (Ctrl+V)
    Event(Event),
}

fn get_config() -> Option<&'static ResidentConfigData> {
    RESIDENT_CONFIG.get()
}

fn resident_update(state: &mut ResidentClaudeInput, message: ResidentMessage) -> Task<ResidentMessage> {
    match message {
        ResidentMessage::EditorAction(action) => {
            state.content.perform(action);
            Task::none()
        }
        ResidentMessage::Submit => {
            let input_text = state.content.text();
            if !input_text.trim().is_empty() {
                // Write to clipboard
                if let Err(e) = clipboard_utils::write_to_clipboard(&input_text) {
                    state.status_message = Some(format!("Clipboard error: {}", e));
                    return Task::none();
                }

                // Paste directly to terminal
                let hwnd = get_config().and_then(|c| c.terminal_hwnd);
                logger::log(&format!("[DEBUG app] terminal_hwnd from config: {:?}", hwnd));
                if let Err(e) = terminal::paste_to_terminal(hwnd) {
                    state.status_message = Some(format!("Send error: {}", e));
                    return Task::none();
                }

                // Clear input
                state.content = text_editor::Content::new();
                state.status_message = Some("Sent!".to_string());
            }
            Task::none()
        }
        ResidentMessage::Event(event) => {
            // TODO: Auto-focus text editor when window is focused
            // iced 0.14 API for text_editor focus needs investigation
            if let Event::Window(window::Event::Focused) = event {
                logger::log("[DEBUG app] Window focused");
            }

            // Handle Ctrl+I to toggle focus back to terminal
            if let Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Character(c),
                modifiers,
                ..
            }) = &event
            {
                if c.as_str() == "i" && modifiers.control() {
                    if let Some(hwnd) = get_config().and_then(|c| c.terminal_hwnd) {
                        let _ = terminal::set_foreground_window(hwnd);
                    }
                }
            }

            // Handle Ctrl+Enter to send
            // Note: We use KeyReleased because text_editor consumes KeyPressed for Enter
            if let Event::Keyboard(keyboard::Event::KeyReleased {
                key: Key::Named(keyboard::key::Named::Enter),
                modifiers,
                ..
            }) = event
            {
                if modifiers.control() {
                    logger::log("[DEBUG app] Ctrl+Enter released, submitting");
                    return resident_update(state, ResidentMessage::Submit);
                }
            }
            Task::none()
        }
    }
}

fn resident_view(state: &ResidentClaudeInput) -> Element<'_, ResidentMessage> {
    // Text editor with Catppuccin Mocha styling
    let editor = text_editor(&state.content)
        .placeholder("Enter your prompt... (Ctrl+I: Toggle | Ctrl+Enter: Send)")
        .on_action(ResidentMessage::EditorAction)
        .height(Length::Fill)
        .padding(10)
        .style(|_theme: &Theme, _status| text_editor::Style {
            background: Background::Color(Color::from_rgb8(49, 50, 68)),   // Surface0
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb8(69, 71, 90),  // Surface1
            },
            placeholder: Color::from_rgb8(108, 112, 134), // Overlay0
            value: Color::from_rgb8(205, 214, 244),       // Text
            selection: Color::from_rgba8(137, 180, 250, 0.4),  // Blue with 40% opacity
        });

    // Status message (Catppuccin colors)
    let status = if let Some(ref msg) = state.status_message {
        if msg.contains("error") || msg.contains("Error") {
            text(msg).size(12).color(Color::from_rgb8(243, 139, 168))  // Red
        } else {
            text(msg).size(12).color(Color::from_rgb8(166, 227, 161))  // Green
        }
    } else {
        text("").size(12)
    };

    // Status bar
    let status_bar = container(status)
        .padding([5, 10]);

    let content = column![
        editor,
        status_bar,
    ]
    .spacing(10)
    .padding(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(30, 30, 46))), // Base
            ..Default::default()
        })
        .into()
}

fn resident_subscription(_state: &ResidentClaudeInput) -> Subscription<ResidentMessage> {
    // Note: Pulse animation disabled for now (time::every not available in iced 0.14)
    // Just use static highlight when typing - can add animation later
    event::listen().map(ResidentMessage::Event)
}

/// Run the GUI application in resident mode
pub fn run_resident_gui(config: ResidentConfig) -> iced::Result {
    // Store config globally (OnceLock ensures thread-safe one-time initialization)
    let _ = RESIDENT_CONFIG.set(ResidentConfigData {
        session_id: config.session_id,
        cwd: config.cwd,
        label: config.label,
        terminal_hwnd: config.terminal_hwnd,
    });

    // Load window icon from PNG
    let icon = window::icon::from_file_data(
        include_bytes!("../assets/MojiBridge-Icon.png"),
        None,
    ).ok();

    iced::application(
        || (ResidentClaudeInput::default(), Task::none()),
        resident_update,
        resident_view,
    )
    .title("MojiBridge")
    .subscription(resident_subscription)
    .window_size(Size::new(600.0, 350.0))
    .window(window::Settings {
        icon,
        ..Default::default()
    })
    .theme(resident_theme)
    .font(include_bytes!("../assets/NotoSansJP-Regular.ttf").as_slice())
    .default_font(Font::with_name("Noto Sans JP"))
    .run()
}

// ============================================
// Legacy one-shot mode (backward compatibility)
// ============================================

/// The main application state for one-shot mode
#[derive(Default)]
pub struct ClaudeInput {
    content: text_editor::Content,
}

/// Messages for the one-shot application
#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(text_editor::Action),
    Submit,
    Cancel,
    Event(Event),
}

fn update(state: &mut ClaudeInput, message: Message) -> Task<Message> {
    match message {
        Message::EditorAction(action) => {
            state.content.perform(action);
            Task::none()
        }
        Message::Submit => {
            let input_text = state.content.text();
            if !input_text.trim().is_empty() {
                // Write output to stdout
                if let Err(e) = hook::write_hook_output(&input_text) {
                    eprintln!("Error writing hook output: {}", e);
                }
            }
            window::latest().and_then(window::close)
        }
        Message::Cancel => {
            window::latest().and_then(window::close)
        }
        Message::Event(event) => {
            // Handle Ctrl+Enter
            if let Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(keyboard::key::Named::Enter),
                modifiers,
                ..
            }) = event
            {
                if modifiers.control() {
                    let input_text = state.content.text();
                    if !input_text.trim().is_empty() {
                        if let Err(e) = hook::write_hook_output(&input_text) {
                            eprintln!("Error writing hook output: {}", e);
                        }
                    }
                    return window::latest().and_then(window::close);
                }
            }
            Task::none()
        }
    }
}

fn view(state: &ClaudeInput) -> Element<'_, Message> {
    let editor = text_editor(&state.content)
        .placeholder("Enter your prompt here...")
        .on_action(Message::EditorAction)
        .height(Length::Fill)
        .padding(10);

    let cancel_button = button(text("Cancel").size(14))
        .padding([8, 16])
        .on_press(Message::Cancel);

    let submit_button = button(text("Submit").size(14))
        .padding([8, 16])
        .on_press(Message::Submit);

    let buttons = row![cancel_button, submit_button].spacing(10);

    let content = column![
        text("Enter your prompt:").size(18),
        editor,
        buttons,
    ]
    .spacing(15)
    .padding(20);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn subscription(_state: &ClaudeInput) -> Subscription<Message> {
    event::listen().map(Message::Event)
}

/// Run the GUI application in one-shot mode (legacy)
pub fn run_gui() -> iced::Result {
    // Load window icon from PNG
    let icon = window::icon::from_file_data(
        include_bytes!("../assets/MojiBridge-Icon.png"),
        None,
    ).ok();

    iced::application(
        || (ClaudeInput::default(), Task::none()),
        update,
        view,
    )
    .title("MojiBridge")
    .subscription(subscription)
    .window_size(Size::new(500.0, 300.0))
    .window(window::Settings {
        icon,
        ..Default::default()
    })
    .font(include_bytes!("../assets/NotoSansJP-Regular.ttf").as_slice())
    .default_font(Font::with_name("Noto Sans JP"))
    .run()
}
