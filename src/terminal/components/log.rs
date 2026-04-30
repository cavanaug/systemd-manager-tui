use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, List, ListItem},
    Frame,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;
use textwrap::wrap;
use rayon::prelude::*;

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use crate::usecases::services_manager::ServicesManager;

fn render_loading(frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL);

    frame.render_widget(block.clone(), area);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Length(1),
            Constraint::Percentage(45),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(vertical[1]);

    let loading = Paragraph::new("Loading...").alignment(Alignment::Center);

    frame.render_widget(loading, horizontal[1]);
}

enum BorderColor {
    White,
    Orange,
}

fn page_jump(visible_height: u16) -> u16 {
    visible_height.saturating_sub(2).max(1)
}

fn half_page(visible_height: u16) -> u16 {
    (visible_height / 2).max(1)
}

impl BorderColor {
    fn to_color(&self) -> Color {
        match self {
            BorderColor::White => Color::White,
            BorderColor::Orange => Color::Rgb(255, 165, 0),
        }
    }
}

pub struct ServiceLog {
    border_color: BorderColor,
    service_name: String,
    scroll: u16,
    visible_height: u16,
    sender: Sender<AppEvent>,
    auto_refresh: Arc<Mutex<bool>>,
    usecase: Rc<RefCell<ServicesManager>>,
    log: String,
    full_log: bool,
}

impl ServiceLog {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        Self {
            border_color: BorderColor::White,
            service_name: String::new(),
            scroll: 0,
            sender,
            auto_refresh: Arc::new(Mutex::new(false)),
            usecase,
            log: String::new(),
            full_log: false,
            visible_height: 0,
        }
    }


    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.log.is_empty()  {
            render_loading(frame, area);
            return;
        }

        let width = area.width.saturating_sub(2) as usize;

        let log_lines: Vec<ListItem> = self
            .log
            .lines()
            .flat_map(|line| {
                wrap(line,width)
                    .into_par_iter()
                    .map(|wrapped| ListItem::new(Span::raw(wrapped.into_owned())))
                    .collect::<Vec<_>>()
            })
            .collect();

        let total_lines = log_lines.len();
        let height = area.height.saturating_sub(2) as usize;
        self.visible_height = height as u16;

        let start = total_lines.saturating_sub(height + self.scroll as usize);
        let end = (start + height).min(total_lines);

        let log_lines: Vec<ListItem> = log_lines[start..end].to_vec();

        let log_list = 
            List::new(log_lines)
                .block(
                    Block::default()
                        .title(format!(" {} log ", self.service_name))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(self.border_color.to_color()))
                        .title_alignment(Alignment::Center),
                );

        frame.render_widget(log_list, area);
    }

    fn toogle_auto_refresh(&mut self) {
        let new_value = {
            if let Ok(auto) = self.auto_refresh.lock() {
                !*auto
            } else {
                return;
            }
        };

        self.set_auto_refresh(new_value);
    }

    fn set_auto_refresh(&mut self, value: bool) {
        self.border_color = if value {
            BorderColor::Orange
        } else {
            BorderColor::White
        };

        if let Ok(mut auto) = self.auto_refresh.lock() {
            *auto = value;
        }
    }

    pub fn set_full_log(&mut self, value: bool) {
        self.full_log = value;
    }

    fn scroll_up(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let up_keys = [KeyCode::Up, KeyCode::Char('k')];
        let down_keys = [KeyCode::Down, KeyCode::Char('j')];

        match key.code {
            code if right_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
            }
            code if left_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
            }
            code if up_keys.contains(&code) => {
                self.scroll_up(1);
            }
            code if down_keys.contains(&code) => {
                self.scroll_down(1);
            }
            KeyCode::PageUp => {
                self.scroll_up(page_jump(self.visible_height));
            }
            KeyCode::PageDown => {
                self.scroll_down(page_jump(self.visible_height));
            }
            KeyCode::Char('b') => {
                self.scroll_up(page_jump(self.visible_height));
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.scroll_down(page_jump(self.visible_height));
            }
            KeyCode::Char('u') => {
                self.scroll_up(half_page(self.visible_height));
            }
            KeyCode::Char('d') => {
                self.scroll_down(half_page(self.visible_height));
            }
            KeyCode::Char('g') | KeyCode::Char('<') => {
                self.scroll = u16::MAX;
            }
            KeyCode::Char('G') | KeyCode::Char('>') => {
                self.scroll = 0;
            }
            KeyCode::Char('a') => {
                self.toogle_auto_refresh();
                self.auto_refresh_thread();
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.reset();
                self.exit();
            }
            _ => {}
        }
    }

    pub fn shortcuts(&self) -> Vec<Line<'_>> {
        let is_refreshing = self.auto_refresh.lock().map(|r| *r).unwrap_or(false);
        let mut auto_refresh_label = "Enable auto-refresh";
        if is_refreshing {
            auto_refresh_label = "Disable auto-refresh";
        }

        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(format!(
                "↑/↓ | u/d (half) | b/f/Space (page) | PgUp/PgDn | g/< top | G/> bottom | {auto_refresh_label}: a | Back: q/Esc",
            )),
        ];

        help_text
    }

    pub fn reset(&mut self) {
        self.set_auto_refresh(false);
        self.scroll = 0;
        self.log = String::new();
        self.full_log = false;
    }

    fn exit(&mut self) {
        self.sender.send(AppEvent::Action(Actions::GoList)).unwrap();
    }

    pub fn auto_refresh_thread(&mut self) {
        let auto_refresh = Arc::clone(&self.auto_refresh);
        let sender = self.sender.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(1000));
                if let Ok(is_active) = auto_refresh.lock() {
                    if *is_active {
                        sender.send(AppEvent::Action(Actions::RefreshLog)).unwrap();
                    } else {
                        break;
                    }
                }
            }
        });
    }

    pub fn fetch_log_and_dispatch(&mut self, service: &Service) {
        let event_tx = self.sender.clone();
        let result = if self.full_log {
            self.usecase.borrow().get_log_full(service)
        } else {
            self.usecase.borrow().get_log(service)
        };
        if let Ok(log) = result {
            event_tx
                .send(AppEvent::Action(Actions::Updatelog((
                    service.name().to_string(),
                    log,
                ))))
                .expect("Failed to send Updatelog event");
        }
    }

    pub fn update(&mut self, service_name: String, log: String) {
        self.service_name = service_name;
        self.log = log;
    }

}
