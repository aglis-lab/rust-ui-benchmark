//! Xilem version of the benchmark app. Compare with `iced-bench/src/main.rs`:
//! same pages, same widgets, same shared state from `bench-core`.

use std::time::Duration;

use bench_core::{
    palette, AppModel, Density, Filter, Item, Page, Stat, GAP, LANGUAGES, PADDING, RADIUS,
    SHADOW_BLUR, SHADOW_OFFSET_Y,
};
use xilem::core::fork;
use xilem::core::one_of::OneOf3;
use xilem::dpi::LogicalSize;
use xilem::masonry::properties::types::{AsUnit, CrossAxisAlignment};
use xilem::masonry::properties::BoxShadow;
use xilem::style::Style as _;
use xilem::tokio::time;
use xilem::view::{
    button, checkbox, flex_col, flex_row, label, portal, progress_bar, sized_box, slider, task,
    text_button, text_input, FlexExt as _,
};
use xilem::winit::error::EventLoopError;
use xilem::{Color, EventLoop, WidgetView, WindowOptions, Xilem};

fn main() -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(
        AppModel::default(),
        view,
        WindowOptions::new("xilem bench").with_initial_inner_size(LogicalSize::new(1000.0, 720.0)),
    );
    app.run_in(EventLoop::with_user_event())
}

// ---- helpers ---------------------------------------------------------------

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgb8(c[0], c[1], c[2])
}

fn gap() -> xilem::masonry::properties::types::Length {
    (GAP as f64).px()
}

/// Rounded, bordered card with an optional soft shadow.
///
/// WORKAROUND: in Masonry 0.4 only `Button` and `TextInput` support
/// `BoxShadow` (SizedBox and Flex do not), so the card container is a no-op
/// `button`. Its background/border are pinned so hover and press look static.
/// iced's `container` supports shadows natively, so this is a real gap.
fn card<V: WidgetView<AppModel>>(inner: V, shadows: bool) -> impl WidgetView<AppModel> + use<V> {
    let shadow = if shadows {
        BoxShadow::new(
            Color::from_rgba8(0, 0, 0, palette::SHADOW_ALPHA),
            (0.0, SHADOW_OFFSET_Y as f64),
        )
        .blur(SHADOW_BLUR as f64)
    } else {
        // fully transparent shadow is treated as "no shadow" by Masonry
        BoxShadow::new(Color::from_rgba8(0, 0, 0, 0), (0.0, 0.0))
    };
    button(sized_box(inner).expand_width(), |_: &mut AppModel| {})
        .padding(PADDING as f64)
        .background_color(rgb(palette::CARD))
        .active_background_color(rgb(palette::CARD))
        .border(rgb(palette::BORDER), 1.0)
        .hovered_border_color(rgb(palette::BORDER))
        .corner_radius(RADIUS as f64)
        .box_shadow(shadow)
}

/// In Masonry a flex child only gets a *maximum* size, so content-sized widgets
/// never grow to fill their share. Wrapping them in an expanding `sized_box`
/// turns that maximum into a real width (the equivalent of iced's `Length::Fill`).
fn fill<V: WidgetView<AppModel>>(v: V) -> impl WidgetView<AppModel> + use<V> {
    sized_box(v).expand_width()
}

fn muted(s: String) -> impl WidgetView<AppModel> + use<> {
    label(s).text_size(12.0).color(rgb(palette::MUTED))
}

/// A row of buttons where the selected one is disabled (same trick as iced).
fn choice_buttons<T: Copy + PartialEq + Send + Sync + 'static>(
    options: Vec<(T, &'static str)>,
    selected: T,
    on_pick: fn(&mut AppModel, T),
) -> impl WidgetView<AppModel> + use<T> {
    let buttons = options
        .into_iter()
        .map(|(value, name)| {
            text_button(name, move |m: &mut AppModel| on_pick(m, value)).disabled(value == selected)
        })
        .collect::<Vec<_>>();
    flex_row(buttons).gap(gap())
}

// ---- root view -------------------------------------------------------------

fn view(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    let nav = flex_row(
        Page::ALL
            .iter()
            .map(|&p| {
                text_button(p.title(), move |m: &mut AppModel| m.page = p).disabled(m.page == p)
            })
            .collect::<Vec<_>>(),
    )
    .gap(gap());

    // Only the active page is built, like iced's `match`.
    let page = match m.page {
        Page::List => OneOf3::A(list_page(m)),
        Page::Dashboard => OneOf3::B(dashboard_page(m)),
        Page::Settings => OneOf3::C(settings_page(m)),
    };

    let content = sized_box(
        flex_col((nav, page.flex(1.0)))
            .gap((GAP as f64 * 1.5).px())
            .cross_axis_alignment(CrossAxisAlignment::Fill),
    )
    .expand()
    .padding(PADDING as f64 * 1.5)
    .background_color(rgb(palette::WINDOW));

    // 60 Hz ticker, only alive while "Live animation" is on.
    fork(
        content,
        m.live.then(|| {
            task(
                |proxy| async move {
                    let mut interval = time::interval(Duration::from_millis(16));
                    loop {
                        interval.tick().await;
                        if proxy.message(()).is_err() {
                            break;
                        }
                    }
                },
                |m: &mut AppModel, ()| m.tick(),
            )
        }),
    )
}

// ---- page 1: list (1000 rows) ----------------------------------------------

fn list_page(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    let toolbar = flex_row((
        fill(
            text_input(m.search.clone(), |m: &mut AppModel, s| m.search = s)
                .placeholder("Search..."),
        )
        .flex(1.0),
        choice_buttons(
            Filter::ALL.iter().map(|&f| (f, f.label())).collect(),
            m.filter,
            |m, f| m.filter = f,
        ),
    ))
    .must_fill_major_axis(true)
    .gap(gap())
    .cross_axis_alignment(CrossAxisAlignment::Center);

    let summary = muted(format!(
        "Showing {} of {} items",
        m.visible_count(),
        m.items.len()
    ));

    flex_col((toolbar, summary, list(m).flex(1.0)))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Fill)
}

#[cfg(not(feature = "virtual-list"))]
fn list(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    let shadows = m.shadows;
    let rows = m
        .visible_items()
        .map(|it| item_row(it, shadows))
        .collect::<Vec<_>>();
    portal(
        sized_box(
            flex_col(rows)
                .gap((m.row_gap() as f64).px())
                .cross_axis_alignment(CrossAxisAlignment::Fill),
        )
        .padding(PADDING as f64),
    )
}

#[cfg(feature = "virtual-list")]
fn list(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    use xilem::core::one_of::Either;
    use xilem::view::virtual_scroll;
    let count = m.visible_count() as i64;
    virtual_scroll(0..count, |m: &mut AppModel, idx| {
        let shadows = m.shadows;
        match m.visible_items().nth(idx as usize) {
            Some(it) => Either::A(sized_box(item_row(it, shadows)).padding(PADDING as f64 / 2.0)),
            None => Either::B(sized_box(label(""))),
        }
    })
}

fn item_row(it: &Item, shadows: bool) -> impl WidgetView<AppModel> + use<> {
    let id = it.id;
    card(
        flex_row((
            checkbox("Done", it.done, move |m: &mut AppModel, d| m.set_done(id, d)),
            fill(
                flex_col((
                    label(it.title.clone()).text_size(16.0),
                    muted(it.subtitle.clone()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Start),
            )
            .flex(3.0),
            fill(progress_bar(Some(it.value))).flex(2.0),
            text_button(
                if it.starred { "★" } else { "☆" },
                move |m: &mut AppModel| m.toggle_star(id),
            ),
        ))
        .must_fill_major_axis(true)
        .gap((GAP as f64 * 2.0).px())
        .cross_axis_alignment(CrossAxisAlignment::Center),
        shadows,
    )
}

// ---- page 2: dashboard -----------------------------------------------------

fn dashboard_page(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    let header = flex_row((
        fill(label("Dashboard").text_size(24.0)).flex(1.0),
        checkbox("Live animation", m.live, |m: &mut AppModel, b| m.live = b),
        text_button("Randomize", |m: &mut AppModel| m.randomize_dashboard()),
        text_button("Reset", |m: &mut AppModel| m.reset_dashboard()),
    ))
    .must_fill_major_axis(true)
    .gap(gap())
    .cross_axis_alignment(CrossAxisAlignment::Center);

    let [a, b, c, d, e, f] = m.stats();
    let s = m.shadows;
    let cards = flex_col((
        flex_row((
            stat_card(a, s).flex(1.0),
            stat_card(b, s).flex(1.0),
            stat_card(c, s).flex(1.0),
        ))
        .must_fill_major_axis(true)
        .gap(gap()),
        flex_row((
            stat_card(d, s).flex(1.0),
            stat_card(e, s).flex(1.0),
            stat_card(f, s).flex(1.0),
        ))
        .must_fill_major_axis(true)
        .gap(gap()),
    ))
    .gap(gap())
    .cross_axis_alignment(CrossAxisAlignment::Fill);

    let sliders = card(
        flex_col((
            slider_row("Volume", m.volume, |m, v| m.volume = v),
            slider_row("Brightness", m.brightness, |m, v| m.brightness = v),
            slider_row("CPU limit", m.cpu_limit, |m, v| m.cpu_limit = v),
        ))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Fill),
        m.shadows,
    );

    flex_col((header, cards, sliders))
        .gap((GAP as f64 * 1.5).px())
        .cross_axis_alignment(CrossAxisAlignment::Fill)
}

fn stat_card(s: Stat, shadows: bool) -> impl WidgetView<AppModel> + use<> {
    card(
        flex_col((
            muted(s.title.to_string()),
            label(s.value_text).text_size(28.0),
            progress_bar(Some(s.fraction)),
        ))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Fill),
        shadows,
    )
}

fn slider_row(
    name: &'static str,
    value: f64,
    on_change: fn(&mut AppModel, f64),
) -> impl WidgetView<AppModel> + use<> {
    flex_row((
        sized_box(label(name)).width(100.px()),
        fill(slider(0.0, 100.0, value, move |m: &mut AppModel, v| on_change(m, v))).flex(1.0),
        sized_box(label(format!("{value:.0}%"))).width(60.px()),
    ))
    .must_fill_major_axis(true)
    .gap(gap())
    .cross_axis_alignment(CrossAxisAlignment::Center)
}

// ---- page 3: settings ------------------------------------------------------

fn settings_page(m: &mut AppModel) -> impl WidgetView<AppModel> + use<> {
    let profile = card(
        flex_col((
            label("Profile").text_size(18.0),
            labeled(
                "Username",
                text_input(m.username.clone(), |m: &mut AppModel, s| m.username = s)
                    .placeholder("username"),
            ),
            labeled(
                "Email",
                text_input(m.email.clone(), |m: &mut AppModel, s| m.email = s)
                    .placeholder("email"),
            ),
        ))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Fill),
        m.shadows,
    );

    let connectivity = card(
        flex_col((
            label("Connectivity").text_size(18.0),
            checkbox("Notifications", m.notifications, |m: &mut AppModel, b| {
                m.notifications = b
            }),
            checkbox("Wi-Fi", m.wifi, |m: &mut AppModel, b| m.wifi = b),
            checkbox("Bluetooth", m.bluetooth, |m: &mut AppModel, b| m.bluetooth = b),
        ))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Start),
        m.shadows,
    );

    let appearance = card(
        flex_col((
            label("Appearance").text_size(18.0),
            checkbox("Card shadows", m.shadows, |m: &mut AppModel, b| m.shadows = b),
            labeled(
                "Density",
                choice_buttons(
                    Density::ALL.iter().map(|&d| (d, d.label())).collect(),
                    m.density,
                    |m, d| m.density = d,
                ),
            ),
            labeled(
                "Language",
                choice_buttons(
                    LANGUAGES.iter().copied().enumerate().collect(),
                    m.language,
                    |m, i| m.language = i,
                ),
            ),
        ))
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Start),
        m.shadows,
    );

    portal(
        sized_box(
            flex_col((profile, connectivity, appearance))
                .gap((GAP as f64 * 1.5).px())
                .cross_axis_alignment(CrossAxisAlignment::Fill),
        )
        .padding(PADDING as f64),
    )
}

fn labeled<V: WidgetView<AppModel>>(name: &'static str, field: V) -> impl WidgetView<AppModel> + use<V> {
    flex_row((sized_box(label(name)).width(100.px()), fill(field).flex(1.0)))
        .must_fill_major_axis(true)
        .gap(gap())
        .cross_axis_alignment(CrossAxisAlignment::Center)
}
