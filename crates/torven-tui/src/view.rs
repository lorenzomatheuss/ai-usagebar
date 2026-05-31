//! TUI rendering — tabs + body + footer.
//!
//! Story 1.4 dropped the `Theme` parameter — colors now come from the fixed
//! palette in [`crate::format_tui`].

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use torven_core::vendor::VendorId;

use crate::app::App;
use crate::panels;

/// Accent color for active UI chrome — calm green matches macOS system blue
/// closely enough to feel native in a terminal.
const ACCENT: Color = Color::Rgb(0x4D, 0x9D, 0xE0);
const DIM: Color = Color::Rgb(0x6B, 0x72, 0x80);
const FG: Color = Color::Rgb(0xE5, 0xE7, 0xEB);

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(1),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    // Settings overlay sits on top — rendered last so it covers everything.
    if let Some(s) = &app.settings {
        crate::settings::render(f, f.area(), s);
    }
}

fn vendor_label(id: VendorId) -> &'static str {
    match id {
        VendorId::Anthropic => "Claude",
        VendorId::Openai => "OpenAI",
        VendorId::Zai => "GLM (Z.AI)",
        VendorId::Openrouter => "OpenRouter",
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let titles: Vec<Line> = app
        .vendors
        .iter()
        .map(|v| Line::from(vendor_label(*v)))
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" torven ")
        .border_style(Style::default().fg(ACCENT));

    let tabs = Tabs::new(titles)
        .block(block)
        .select(app.active)
        .style(Style::default().fg(FG))
        .highlight_style(
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(" · ");
    f.render_widget(tabs, area);
}

fn draw_body(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(tab) = app.tabs.get(app.active) else {
        return;
    };
    let sections = panels::sections_for(tab, chrono::Utc::now(), 5);
    panels::render(f, inner, &sections);
}

fn draw_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let text = Line::from(vec![
        Span::styled(" [Tab/h-l]", Style::default().fg(ACCENT)),
        Span::styled(" switch · ", Style::default().fg(DIM)),
        Span::styled("[r]", Style::default().fg(ACCENT)),
        Span::styled(" refresh · ", Style::default().fg(DIM)),
        Span::styled("[s]", Style::default().fg(ACCENT)),
        Span::styled(" settings · ", Style::default().fg(DIM)),
        Span::styled("[q]", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
        Span::styled(
            format!("   ·   updated {}", app.last_refresh.format("%H:%M:%S")),
            Style::default().fg(DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

/// Re-exported palette constants so sibling modules (`panels`, `settings`)
/// share the same fixed theme without each duplicating the RGB literals.
pub mod palette {
    use ratatui::style::Color;

    pub const ACCENT: Color = super::ACCENT;
    pub const DIM: Color = super::DIM;
    pub const FG: Color = super::FG;
}
