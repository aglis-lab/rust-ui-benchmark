# iced vs Xilem benchmark apps

Two apps with the same pages, widgets and data. All state and logic live in
`bench-core`, so the only thing that differs is the view code and the toolkit.

```
bench-core/    shared model, update logic, palette (no GUI dependency)
iced-bench/    iced 0.14 UI
xilem-bench/   Xilem 0.4 UI
measure.sh     samples CPU/RSS of a running binary
```

## Pages

1. **List** - 1000 rows. Each row is a shadowed card with a checkbox, two text
   lines, a progress bar and a star button. Search box + All/Starred/Done filter.
2. **Dashboard** - six stat cards (progress bars), three sliders, and a
   **Live animation** checkbox that updates the app at ~60 Hz (worst case for
   idle CPU/GPU).
3. **Settings** - text inputs, checkboxes, button groups, and a
   **Card shadows** checkbox so you can compare with and without shadows.

Only the active page is built in both apps.

## Run

```sh
cargo run --release -p iced-bench
cargo run --release -p xilem-bench
cargo run --release -p xilem-bench --features virtual-list   # Xilem's virtualized list
```

Start straight in a scenario with environment variables:

```sh
BENCH_PAGE=dashboard BENCH_LIVE=1 cargo run --release -p iced-bench
BENCH_PAGE=list BENCH_SHADOWS=0  cargo run --release -p xilem-bench
```

`BENCH_PAGE` = `list` | `dashboard` | `settings`, `BENCH_LIVE` = `1`,
`BENCH_SHADOWS` = `0`.

## Measure

```sh
cargo build --release
BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh target/release/iced-bench  30
BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh target/release/xilem-bench 30
BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --fps target/release/iced-bench  30 iced-fps.csv
BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --fps target/release/xilem-bench 30 xilem-fps.csv
sudo BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --gpu target/release/iced-bench  30 iced-gpu.csv
sudo BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --gpu target/release/xilem-bench 30 xilem-gpu.csv
sudo BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --gpu --fps target/release/iced-bench 30 iced-gpu-fps.csv
sudo BENCH_PAGE=dashboard BENCH_LIVE=1 ./measure.sh --gpu --fps target/release/xilem-bench 30 xilem-gpu-fps.csv
```

Writes a CSV (RSS, CPU %, threads) and prints a summary. On Linux it samples
`/proc/<pid>` once per second; on macOS it falls back to `ps`.
On Apple Silicon, pass `--gpu` to add a `gpu_pct` column using `powermetrics`
(requires `sudo`). Pass `--fps` to add `app_fps` from the app's internal
60 Hz tick loop (`BENCH_FPS_FILE`), which is most meaningful with
`BENCH_PAGE=dashboard BENCH_LIVE=1`. When `--fps` is used, `measure.sh` defaults
`BENCH_LIVE=1` for the child process if not already set. It does **not** measure compositor-presented
frame times; use each toolkit's own frame stats for latency. Interact with the
window while it samples (scroll the list, switch pages).

Suggested matrix: {list, dashboard} x {shadows on, off} x {live on, off},
on your real target hardware, plus one low-end GPU.

## Things that are NOT equal (read before comparing numbers)

- **List virtualization.** iced 0.14 has no virtualized list, so by default
  both apps build and lay out all 1000 rows. Xilem has `virtual_scroll`; enable
  it with `--features virtual-list` to see the difference (that is no longer an
  equal-work comparison).
- **Card shadows in Xilem use a workaround.** In Masonry 0.4 only `Button` and
  `TextInput` accept a box shadow (not `SizedBox` or `Flex`), so each Xilem card
  is a no-op `button` with pinned colors. Nested widgets did render correctly in
  screenshots, but click handling of controls inside a card was **not**
  tested interactively.
- **Slider width.** Masonry 0.4 caps a slider at 200 px
  (`width = max.clamp(100, 200)`), so Xilem's sliders are shorter than iced's.
- **Default widget themes differ.** Xilem's progress bar is thinner and prints a
  percentage; iced's is thicker. Buttons/checkboxes have different default
  looks. Not normalized.
- **Widget set.** Xilem 0.4 has no toggler, radio or pick-list, so both apps use
  only text input, checkbox, button, slider and progress bar.
- **Layout idioms.** Xilem flex children only get a _maximum_ size, so each
  flexed child is wrapped in `sized_box(..).expand_width()` (see `fill()`);
  iced uses `Length::Fill` / `FillPortion`.
- **Shadow visibility.** Both use identical parameters (alpha 130, y-offset 4,
  blur 16) but the shadow is subtle on the dark background.
