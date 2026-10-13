//! iced version of the benchmark app. Compare with `xilem-bench/src/main.rs`:
//! same pages, same widgets, same shared state from `bench-core`.

use std::time::Duration;

use bench_core::{
    palette, AppModel, Density, Filter, Item, Page, Stat, GAP, LANGUAGES, PADDING, RADIUS,
    SHADOW_BLUR, SHADOW_OFFSET_Y,
};
use iced::widget::{
    button, checkbox, column, container, progress_bar, row, scrollable, slider, text, text_input,
    Column,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Subscription, Theme, Vector};

fn main() -> iced::Result {
    iced::application(AppModel::default, update, view)
        .title("iced bench")
        .theme(|_: &AppModel| Theme::Dark)
        .subscription(subscription)
        .window_size((1000.0, 720.0))
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    GoTo(Page),
    // list
    Search(String),
    SetFilter(Filter),
    ToggleStar(usize),
    SetDone(usize, bool),
    // dashboard
    Volume(f64),
    Brightness(f64),
    CpuLimit(f64),
    Live(bool),
    Tick,
    Reset,
    Randomize,
    // settings
    Username(String),
    Email(String),
    Notifications(bool),
    Wifi(bool),
    Bluetooth(bool),
    Shadows(bool),
    SetDensity(Density),
    SetLanguage(usize),
}

fn update(m: &mut AppModel, msg: Message) {
    match msg {
        Message::GoTo(p) => m.page = p,
        Message::Search(s) => m.search = s,
        Message::SetFilter(f) => m.filter = f,
        Message::ToggleStar(id) => m.toggle_star(id),
        Message::SetDone(id, d) => m.set_done(id, d),
        Message::Volume(v) => m.volume = v,
        Message::Brightness(v) => m.brightness = v,
        Message::CpuLimit(v) => m.cpu_limit = v,
        Message::Live(b) => m.live = b,
        Message::Tick => m.tick(),
        Message::Reset => m.reset_dashboard(),
        Message::Randomize => m.randomize_dashboard(),
        Message::Username(s) => m.username = s,
        Message::Email(s) => m.email = s,
        Message::Notifications(b) => m.notifications = b,
        Message::Wifi(b) => m.wifi = b,
        Message::Bluetooth(b) => m.bluetooth = b,
        Message::Shadows(b) => m.shadows = b,
        Message::SetDensity(d) => m.density = d,
        Message::SetLanguage(i) => m.language = i,
    }
}

fn subscription(m: &AppModel) -> Subscription<Message> {
    if m.live {
        iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)
    } else {
        Subscription::none()
    }
}

// ---- helpers ---------------------------------------------------------------

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgb8(c[0], c[1], c[2])
}

/// Rounded, bordered card with an optional soft shadow.
fn card<'a>(content: impl Into<Element<'a, Message>>, shadows: bool) -> Element<'a, Message> {
    container(content)
        .padding(PADDING)
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(rgb(palette::CARD))),
            border: Border {
                color: rgb(palette::BORDER),
                width: 1.0,
                radius: RADIUS.into(),
            },
            shadow: if shadows {
                Shadow {
                    color: Color::from_rgba8(0, 0, 0, palette::SHADOW_ALPHA as f32 / 255.0),
                    offset: Vector::new(0.0, SHADOW_OFFSET_Y),
                    blur_radius: SHADOW_BLUR,
                }
            } else {
                Shadow::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn muted<'a>(s: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(s).size(12).color(rgb(palette::MUTED))
}

/// A row of buttons where the selected one is disabled (same trick as xilem).
fn choice_buttons<'a, T: Copy + PartialEq + 'a>(
    options: impl IntoIterator<Item = (T, &'a str)>,
    selected: T,
    on_pick: impl Fn(T) -> Message,
) -> Element<'a, Message> {
    row(options.into_iter().map(|(value, label)| {
        button(text(label))
            .on_press_maybe((value != selected).then(|| on_pick(value)))
            .into()
    }))
    .spacing(GAP)
    .into()
}

// ---- root view -------------------------------------------------------------

fn view(m: &AppModel) -> Element<'_, Message> {
    let nav = row(Page::ALL.iter().map(|&p| {
        button(text(p.title()))
            .on_press_maybe((m.page != p).then_some(Message::GoTo(p)))
            .into()
    }))
    .spacing(GAP);

    let page: Element<Message> = match m.page {
        Page::List => list_page(m),
        Page::Dashboard => dashboard_page(m),
        Page::Settings => settings_page(m),
    };

    container(column![nav, page].spacing(GAP * 1.5).padding(PADDING * 1.5))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(rgb(palette::WINDOW))),
            ..container::Style::default()
        })
        .into()
}

// ---- page 1: list (1000 rows) ----------------------------------------------

fn list_page(m: &AppModel) -> Element<'_, Message> {
    let toolbar = row![
        text_input("Search...", &m.search)
            .on_input(Message::Search)
            .width(Length::Fill),
        choice_buttons(
            Filter::ALL.iter().map(|&f| (f, f.label())),
            m.filter,
            Message::SetFilter
        ),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);

    let summary = muted(format!(
        "Showing {} of {} items",
        m.visible_count(),
        m.items.len()
    ));

    let rows = Column::with_children(m.visible_items().map(|it| item_row(it, m.shadows)))
        .spacing(m.row_gap())
        .padding(PADDING);

    column![toolbar, summary, scrollable(rows).height(Length::Fill)]
        .spacing(GAP)
        .into()
}

fn item_row(it: &Item, shadows: bool) -> Element<'_, Message> {
    let id = it.id;
    card(
        row![
            checkbox(it.done)
                .label("Done")
                .on_toggle(move |d| Message::SetDone(id, d)),
            column![text(&it.title).size(16), muted(&it.subtitle)]
                .spacing(2)
                .width(Length::FillPortion(3)),
            progress_bar(0.0..=1.0, it.value as f32).length(Length::FillPortion(2)),
            button(text(if it.starred { "★" } else { "☆" })).on_press(Message::ToggleStar(id)),
        ]
        .spacing(GAP * 2.0)
        .align_y(Alignment::Center),
        shadows,
    )
}

// ---- page 2: dashboard -----------------------------------------------------

fn dashboard_page(m: &AppModel) -> Element<'_, Message> {
    let header = row![
        text("Dashboard").size(24).width(Length::Fill),
        checkbox(m.live).label("Live animation").on_toggle(Message::Live),
        button(text("Randomize")).on_press(Message::Randomize),
        button(text("Reset")).on_press(Message::Reset),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);

    let stats = m.stats();
    let [a, b, c, d, e, f] = stats;
    let cards = column![
        row![stat_card(a, m.shadows), stat_card(b, m.shadows), stat_card(c, m.shadows)]
            .spacing(GAP),
        row![stat_card(d, m.shadows), stat_card(e, m.shadows), stat_card(f, m.shadows)]
            .spacing(GAP),
    ]
    .spacing(GAP);

    let sliders = card(
        column![
            slider_row("Volume", m.volume, Message::Volume),
            slider_row("Brightness", m.brightness, Message::Brightness),
            slider_row("CPU limit", m.cpu_limit, Message::CpuLimit),
        ]
        .spacing(GAP),
        m.shadows,
    );

    column![header, cards, sliders].spacing(GAP * 1.5).into()
}

fn stat_card(s: Stat, shadows: bool) -> Element<'static, Message> {
    card(
        column![
            muted(s.title),
            text(s.value_text).size(28),
            progress_bar(0.0..=1.0, s.fraction as f32),
        ]
        .spacing(GAP),
        shadows,
    )
}

fn slider_row<'a>(
    label: &'a str,
    value: f64,
    on_change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(100.0)),
        slider(0.0..=100.0, value, on_change).width(Length::Fill),
        text(format!("{value:.0}%")).width(Length::Fixed(60.0)),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center)
    .into()
}

// ---- page 3: settings ------------------------------------------------------

fn settings_page(m: &AppModel) -> Element<'_, Message> {
    let profile = card(
        column![
            text("Profile").size(18),
            labeled("Username", text_input("username", &m.username).on_input(Message::Username)),
            labeled("Email", text_input("email", &m.email).on_input(Message::Email)),
        ]
        .spacing(GAP),
        m.shadows,
    );

    let connectivity = card(
        column![
            text("Connectivity").size(18),
            checkbox(m.notifications)
                .label("Notifications")
                .on_toggle(Message::Notifications),
            checkbox(m.wifi).label("Wi-Fi").on_toggle(Message::Wifi),
            checkbox(m.bluetooth).label("Bluetooth").on_toggle(Message::Bluetooth),
        ]
        .spacing(GAP),
        m.shadows,
    );

    let appearance = card(
        column![
            text("Appearance").size(18),
            checkbox(m.shadows).label("Card shadows").on_toggle(Message::Shadows),
            labeled(
                "Density",
                choice_buttons(
                    Density::ALL.iter().map(|&d| (d, d.label())),
                    m.density,
                    Message::SetDensity
                )
            ),
            labeled(
                "Language",
                choice_buttons(
                    LANGUAGES.iter().copied().enumerate().map(|(i, l)| (i, l)),
                    m.language,
                    Message::SetLanguage
                )
            ),
        ]
        .spacing(GAP),
        m.shadows,
    );

    scrollable(column![profile, connectivity, appearance].spacing(GAP * 1.5).padding(PADDING))
        .height(Length::Fill)
        .into()
}

fn labeled<'a>(label: &'a str, field: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![text(label).width(Length::Fixed(100.0)), field.into()]
        .spacing(GAP)
        .align_y(Alignment::Center)
        .into()
}
