#!/usr/bin/env bash
# Sample CPU and memory of a running GUI binary.
#
# usage: ./measure.sh [--gpu] [--fps] <binary> [seconds=20] [out.csv=<name>.csv]
#
# Launches the binary, samples once per second, then kills it and prints a
# summary. Interact with the app while it samples (switch pages, scroll the
# list, toggle "Live animation") so you measure what you care about.
#
# Base columns: t, rss_mb, cpu_pct (of one core), threads
# Optional (macOS): --gpu adds gpu_pct from `powermetrics` (requires sudo).
# Optional: --fps adds app_fps from BENCH_FPS_FILE updates (bench apps only).
# Does NOT measure frame times; use the framework's own frame stats for those.
set -euo pipefail

gpu_enabled=0
fps_enabled=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --gpu)
      gpu_enabled=1
      shift
      ;;
    --fps)
      fps_enabled=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "unknown option: $1" >&2
      echo "usage: ./measure.sh [--gpu] [--fps] <binary> [seconds] [out.csv]" >&2
      exit 1
      ;;
    *)
      break
      ;;
  esac
done

bin=${1:?usage: measure.sh [--gpu] [--fps] <binary> [seconds] [out.csv]}
secs=${2:-20}
out=${3:-$(basename "$bin").csv}
fps_file=""

parse_gpu_pct() {
  awk '
    {
      line = tolower($0)
      if (line ~ /gpu/ && match(line, /[0-9]+([.][0-9]+)?%/)) {
        val = substr(line, RSTART, RLENGTH - 1)
      }
      if (line ~ /(active|residency|duty|busy)/ && match(line, /[0-9]+([.][0-9]+)?%/)) {
        val = substr(line, RSTART, RLENGTH - 1)
      }
    }
    END {
      if (val == "") exit 1
      printf "%.1f", val
    }'
}

if (( gpu_enabled )); then
  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "--gpu is currently supported only on macOS (Apple Silicon)." >&2
    exit 1
  fi
  if (( EUID != 0 )); then
    echo "--gpu requires sudo because powermetrics needs superuser privileges." >&2
    echo "Try: sudo ./measure.sh --gpu <binary> [seconds] [out.csv]" >&2
    exit 1
  fi
  if ! command -v powermetrics >/dev/null 2>&1; then
    echo "--gpu requested but 'powermetrics' is not available on this system." >&2
    exit 1
  fi
fi

if (( fps_enabled )); then
  fps_file=$(mktemp -t gui-bench-fps.XXXXXX)
  echo "0.0" >"$fps_file"
  bench_live=${BENCH_LIVE:-1}
  BENCH_FPS_FILE="$fps_file" BENCH_LIVE="$bench_live" "$bin" >/dev/null 2>&1 &
else
  "$bin" >/dev/null 2>&1 &
fi
pid=$!
trap 'if kill -0 "$pid" 2>/dev/null; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi; [[ -n "$fps_file" ]] && rm -f "$fps_file"' EXIT

if (( gpu_enabled && fps_enabled )); then
  echo "t,rss_mb,cpu_pct,threads,gpu_pct,app_fps" >"$out"
elif (( gpu_enabled )); then
  echo "t,rss_mb,cpu_pct,threads,gpu_pct" >"$out"
elif (( fps_enabled )); then
  echo "t,rss_mb,cpu_pct,threads,app_fps" >"$out"
else
  echo "t,rss_mb,cpu_pct,threads" >"$out"
fi

if [[ -r "/proc/$pid/stat" && -r "/proc/$pid/status" ]]; then
  sampler=linux_proc
  ticks=$(getconf CLK_TCK)
  prev_cpu=0
  prev_t=$(date +%s.%N)
else
  sampler=ps
fi

sleep 2   # let the window open before the first sample

for ((i = 1; i <= secs; i++)); do
  kill -0 "$pid" 2>/dev/null || { echo "process exited early" >&2; break; }
  if [[ "$sampler" == "linux_proc" ]]; then
    now=$(date +%s.%N)
    # fields 14,15 of /proc/<pid>/stat = utime, stime (in clock ticks)
    cpu=$(awk '{print $14 + $15}' "/proc/$pid/stat")
    rss_kb=$(awk '/VmRSS/ {print $2}' "/proc/$pid/status")
    thr=$(awk '/Threads/ {print $2}' "/proc/$pid/status")
    pct=$(awk -v c="$cpu" -v p="$prev_cpu" -v n="$now" -v o="$prev_t" -v k="$ticks" \
          'BEGIN { dt = n - o; printf "%.1f", (dt > 0 ? (c - p) / k / dt * 100 : 0) }')
    prev_cpu=$cpu
    prev_t=$now
  else
    line=$(ps -o rss= -o %cpu= -p "$pid" | awk 'NF { print; exit }')
    [[ -n "$line" ]] || { echo "process exited early" >&2; break; }
    rss_kb=$(awk '{print $1}' <<<"$line")
    pct=$(awk '{print $2}' <<<"$line")
    thr=$(ps -M -p "$pid" | awk 'NR > 1 { c++ } END { print c + 0 }')
  fi

  mb=$(awk -v k="$rss_kb" 'BEGIN { printf "%.1f", k / 1024 }')
  fps_val=""
  if (( fps_enabled )); then
    fps_val=$(awk 'NF { v = $1 } END { if (v == "") v = 0; printf "%.1f", v }' "$fps_file")
  fi
  if (( gpu_enabled && fps_enabled )); then
    pm_out=$(powermetrics --samplers gpu_power -n 1 -i 1000 2>&1) || {
      echo "powermetrics failed while sampling GPU." >&2
      echo "$pm_out" >&2
      exit 1
    }
    gpu_pct=$(parse_gpu_pct <<<"$pm_out") || {
      echo "Unable to parse GPU utilization from powermetrics output." >&2
      exit 1
    }
    echo "$i,$mb,$pct,$thr,$gpu_pct,$fps_val" >>"$out"
  elif (( gpu_enabled )); then
    pm_out=$(powermetrics --samplers gpu_power -n 1 -i 1000 2>&1) || {
      echo "powermetrics failed while sampling GPU." >&2
      echo "$pm_out" >&2
      exit 1
    }
    gpu_pct=$(parse_gpu_pct <<<"$pm_out") || {
      echo "Unable to parse GPU utilization from powermetrics output." >&2
      exit 1
    }
    echo "$i,$mb,$pct,$thr,$gpu_pct" >>"$out"
  elif (( fps_enabled )); then
    echo "$i,$mb,$pct,$thr,$fps_val" >>"$out"
    sleep 1
  else
    echo "$i,$mb,$pct,$thr" >>"$out"
    sleep 1
  fi
done

if (( gpu_enabled && fps_enabled )); then
  awk -F, 'NR > 1 { n++; cpu += $3; gpu += $5; fps += $6; nz += ($6 > 0); if ($2 > peak) peak = $2; last = $2 }
           END {
             if (n) {
               printf "samples=%d  avg_cpu=%.1f%%  avg_gpu=%.1f%%  avg_app_fps=%.1f  peak_rss=%.1f MB  last_rss=%.1f MB\n",
                      n, cpu / n, gpu / n, fps / n, peak, last
               if (nz == 0) {
                 print "warning: app_fps stayed 0.0 in all samples; ensure BENCH_LIVE=1 or toggle Live animation on." > "/dev/stderr"
               }
             }
           }' "$out"
elif (( gpu_enabled )); then
  awk -F, 'NR > 1 { n++; cpu += $3; gpu += $5; if ($2 > peak) peak = $2; last = $2 }
           END {
             if (n) {
               printf "samples=%d  avg_cpu=%.1f%%  avg_gpu=%.1f%%  peak_rss=%.1f MB  last_rss=%.1f MB\n",
                      n, cpu / n, gpu / n, peak, last
             }
           }' "$out"
elif (( fps_enabled )); then
  awk -F, 'NR > 1 { n++; cpu += $3; fps += $5; nz += ($5 > 0); if ($2 > peak) peak = $2; last = $2 }
           END {
             if (n) {
               printf "samples=%d  avg_cpu=%.1f%%  avg_app_fps=%.1f  peak_rss=%.1f MB  last_rss=%.1f MB\n",
                      n, cpu / n, fps / n, peak, last
               if (nz == 0) {
                 print "warning: app_fps stayed 0.0 in all samples; ensure BENCH_LIVE=1 or toggle Live animation on." > "/dev/stderr"
               }
             }
           }' "$out"
else
  awk -F, 'NR > 1 { n++; cpu += $3; if ($2 > peak) peak = $2; last = $2 }
           END {
             if (n) {
               printf "samples=%d  avg_cpu=%.1f%%  peak_rss=%.1f MB  last_rss=%.1f MB\n",
                      n, cpu / n, peak, last
             }
           }' "$out"
fi
echo "wrote $out"
