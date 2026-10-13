//! Shared model, update logic and design tokens for the iced and xilem
//! benchmark apps. Both UIs render exactly this state, so any difference in
//! resource usage comes from the toolkit, not from the app logic.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub const ITEM_COUNT: usize = 1000;

// ---- design tokens (identical in both apps) --------------------------------

pub const GAP: f32 = 8.0;
pub const PADDING: f32 = 12.0;
pub const RADIUS: f32 = 10.0;

pub const SHADOW_OFFSET_Y: f32 = 4.0;
pub const SHADOW_BLUR: f32 = 16.0;

/// Colors as 8-bit sRGB triples (+ alpha where relevant).
pub mod palette {
    pub const WINDOW: [u8; 3] = [0x14, 0x16, 0x1c];
    pub const CARD: [u8; 3] = [0x1e, 0x22, 0x2b];
    pub const BORDER: [u8; 3] = [0x2a, 0x2f, 0x3a];
    pub const MUTED: [u8; 3] = [0x8b, 0x93, 0xa5];
    pub const ACCENT: [u8; 3] = [0x5b, 0x8d, 0xef];
    /// Shadow color alpha (0-255), RGB is black.
    pub const SHADOW_ALPHA: u8 = 130;
}

// ---- data ------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Item {
    pub id: usize,
    pub title: String,
    pub subtitle: String,
    /// 0.0..=1.0, drawn as a progress bar in each row.
    pub value: f64,
    pub starred: bool,
    pub done: bool,
}

/// Deterministic fake data so both apps show the same rows.
pub fn make_items(n: usize) -> Vec<Item> {
    const TOPICS: [&str; 8] = [
        "Quarterly report",
        "Design review",
        "Invoice",
        "Server migration",
        "Customer interview",
        "Release notes",
        "Budget draft",
        "Onboarding doc",
    ];
    (0..n)
        .map(|i| Item {
            id: i,
            title: format!("{} #{}", TOPICS[i % TOPICS.len()], i + 1),
            subtitle: format!("Owner: user{:03} · updated {} min ago", i % 97, (i * 7) % 600),
            value: ((i * 37) % 100) as f64 / 100.0,
            starred: i % 5 == 0,
            done: i % 3 == 0,
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    List,
    Dashboard,
    Settings,
}

impl Page {
    pub const ALL: [Page; 3] = [Page::List, Page::Dashboard, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Page::List => "List",
            Page::Dashboard => "Dashboard",
            Page::Settings => "Settings",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    All,
    Starred,
    Done,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Starred, Filter::Done];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Starred => "Starred",
            Filter::Done => "Done",
        }
    }

    pub fn matches(self, item: &Item) -> bool {
        match self {
            Filter::All => true,
            Filter::Starred => item.starred,
            Filter::Done => item.done,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Density {
    Compact,
    Comfortable,
    Spacious,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];

    pub fn label(self) -> &'static str {
        match self {
            Density::Compact => "Compact",
            Density::Comfortable => "Comfortable",
            Density::Spacious => "Spacious",
        }
    }
}

pub const LANGUAGES: [&str; 4] = ["English", "Bahasa Indonesia", "日本語", "Deutsch"];

/// One dashboard card.
pub struct Stat {
    pub title: &'static str,
    pub value_text: String,
    pub fraction: f64,
}

// ---- state + update logic ---------------------------------------------------

pub struct AppModel {
    pub page: Page,

    // list page
    pub items: Vec<Item>,
    pub filter: Filter,
    pub search: String,

    // dashboard page
    pub volume: f64,     // 0..=100
    pub brightness: f64, // 0..=100
    pub cpu_limit: f64,  // 0..=100
    /// When true the app ticks at ~60 Hz and animates the dashboard.
    pub live: bool,
    pub phase: f64,

    // settings page
    pub username: String,
    pub email: String,
    pub notifications: bool,
    pub wifi: bool,
    pub bluetooth: bool,
    pub shadows: bool,
    pub density: Density,
    pub language: usize,
}

struct FpsProbe {
    path: PathBuf,
    window_start: Instant,
    ticks: u32,
    write_failed: bool,
}

impl FpsProbe {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            window_start: Instant::now(),
            ticks: 0,
            write_failed: false,
        }
    }

    fn record_tick(&mut self) {
        self.ticks += 1;
        let elapsed = self.window_start.elapsed().as_secs_f64();
        if elapsed < 0.5 {
            return;
        }
        let fps = self.ticks as f64 / elapsed;
        if let Err(err) = std::fs::write(&self.path, format!("{fps:.1}\n")) {
            if !self.write_failed {
                eprintln!("BENCH_FPS_FILE write failed: {err}");
                self.write_failed = true;
            }
        }
        self.window_start = Instant::now();
        self.ticks = 0;
    }
}

fn fps_probe() -> Option<&'static Mutex<FpsProbe>> {
    static PROBE: OnceLock<Option<Mutex<FpsProbe>>> = OnceLock::new();
    PROBE
        .get_or_init(|| {
            std::env::var_os("BENCH_FPS_FILE")
                .filter(|v| !v.is_empty())
                .map(|v| Mutex::new(FpsProbe::new(PathBuf::from(v))))
        })
        .as_ref()
}

fn record_bench_fps_tick() {
    let Some(probe) = fps_probe() else {
        return;
    };
    match probe.lock() {
        Ok(mut guard) => guard.record_tick(),
        Err(err) => {
            eprintln!("BENCH_FPS_FILE state poisoned: {err}");
        }
    }
}

fn env_flag(name: &str) -> Option<bool> {
    std::env::var(name)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

impl Default for AppModel {
    /// Start state. Environment overrides let you launch straight into a
    /// benchmark scenario:
    ///   BENCH_PAGE=list|dashboard|settings   BENCH_LIVE=1   BENCH_SHADOWS=0
    fn default() -> Self {
        Self {
            page: match std::env::var("BENCH_PAGE").as_deref() {
                Ok("dashboard") => Page::Dashboard,
                Ok("settings") => Page::Settings,
                _ => Page::List,
            },
            items: make_items(ITEM_COUNT),
            filter: Filter::All,
            search: String::new(),
            volume: 60.0,
            brightness: 80.0,
            cpu_limit: 45.0,
            live: env_flag("BENCH_LIVE").unwrap_or(false),
            phase: 0.0,
            username: "gooner".into(),
            email: "gooner@example.com".into(),
            notifications: true,
            wifi: true,
            bluetooth: false,
            shadows: env_flag("BENCH_SHADOWS").unwrap_or(true),
            density: Density::Comfortable,
            language: 0,
        }
    }
}

impl AppModel {
    /// Rows that pass the current search + filter, in order.
    pub fn visible_items(&self) -> impl Iterator<Item = &Item> {
        let needle = self.search.to_lowercase();
        self.items.iter().filter(move |it| {
            self.filter.matches(it) && (needle.is_empty() || it.title.to_lowercase().contains(&needle))
        })
    }

    pub fn visible_count(&self) -> usize {
        self.visible_items().count()
    }

    pub fn toggle_star(&mut self, id: usize) {
        if let Some(it) = self.items.get_mut(id) {
            it.starred = !it.starred;
        }
    }

    pub fn set_done(&mut self, id: usize, done: bool) {
        if let Some(it) = self.items.get_mut(id) {
            it.done = done;
        }
    }

    /// Vertical gap between list rows, driven by the density setting.
    pub fn row_gap(&self) -> f32 {
        match self.density {
            Density::Compact => 4.0,
            Density::Comfortable => 8.0,
            Density::Spacious => 16.0,
        }
    }

    pub fn tick(&mut self) {
        self.phase += 0.05;
        record_bench_fps_tick();
    }

    pub fn reset_dashboard(&mut self) {
        self.volume = 60.0;
        self.brightness = 80.0;
        self.cpu_limit = 45.0;
        self.phase = 0.0;
    }

    pub fn randomize_dashboard(&mut self) {
        // cheap deterministic "random" so there is no rand dependency
        self.volume = (self.volume * 7.0 + 31.0) % 100.0;
        self.brightness = (self.brightness * 5.0 + 17.0) % 100.0;
        self.cpu_limit = (self.cpu_limit * 3.0 + 53.0) % 100.0;
    }

    /// The six stat cards. With `live` on these move every tick.
    pub fn stats(&self) -> [Stat; 6] {
        let wave = |offset: f64| (((self.phase + offset).sin() + 1.0) / 2.0).clamp(0.0, 1.0);
        let done = self.items.iter().filter(|i| i.done).count() as f64;
        let starred = self.items.iter().filter(|i| i.starred).count() as f64;
        let total = self.items.len().max(1) as f64;
        let mk = |title, f: f64, text: String| Stat { title, value_text: text, fraction: f };
        [
            mk("Completed", done / total, format!("{done:.0}")),
            mk("Starred", starred / total, format!("{starred:.0}")),
            mk("Volume", self.volume / 100.0, format!("{:.0}%", self.volume)),
            mk("CPU", wave(0.0), format!("{:.0}%", wave(0.0) * 100.0)),
            mk("Memory", wave(1.5), format!("{:.0}%", wave(1.5) * 100.0)),
            mk("Network", wave(3.0), format!("{:.0}%", wave(3.0) * 100.0)),
        ]
    }
}
