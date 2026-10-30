#!/usr/bin/env bash
# Claude Code status line script

input=$(cat)

# Extract fields from JSON input
cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // "?"')
model=$(echo "$input" | jq -r '.model.display_name // "?"')
effort=$(echo "$input" | jq -r '.effort.level // empty')
used=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
input_tokens=$(echo "$input" | jq -r '.context_window.total_input_tokens // empty')
output_tokens=$(echo "$input" | jq -r '.context_window.total_output_tokens // empty')
lines_added=$(echo "$input" | jq -r '.cost.total_lines_added // empty')
lines_removed=$(echo "$input" | jq -r '.cost.total_lines_removed // empty')

report_herdr_model() {
  [ "${HERDR_ENV:-}" = "1" ] && [ -n "${HERDR_PANE_ID:-}" ] || return 0

  local herdr_model
  herdr_model=$(echo "$input" | jq -r '
    (
      (.model.display_name | select(type == "string" and length > 0 and . != "?"))
      // (.model.id | select(type == "string" and length > 0 and . != "?"))
      // ""
    )
    | gsub("\u001b\\[[0-?]*[ -/]*[@-~]"; "")
    | gsub("[[:cntrl:]]"; " ")
    | gsub("[[:space:]]+"; " ")
    | sub("^ +"; "")
    | sub(" +$"; "")
    | explode
    | if length > 80 then .[:79] + [8230] else . end
    | implode
  ')

  if ! command -v timeout > /dev/null 2>&1; then
    printf '%s\n' 'Claude statusline: cannot report the model because timeout is unavailable.' >&2
    return 0
  fi

  local herdr_bin=${HERDR_BIN_PATH:-herdr}
  local token_args=(--clear-token claude_model)
  if [ -n "$herdr_model" ]; then
    token_args=(--token "claude_model=${herdr_model}")
  fi

  if ! timeout 1s "$herdr_bin" pane report-metadata "$HERDR_PANE_ID" \
    --source claude:herdr-model \
    --agent claude \
    --ttl-ms 45000 \
    "${token_args[@]}" > /dev/null 2>&1; then
    printf '%s\n' 'Claude statusline: failed to report the model to Herdr.' >&2
  fi
}

report_herdr_model

IFS=$'\t' read -r weekly_remaining weekly_filled weekly_reset < <(
  echo "$input" | jq -r '
    .rate_limits.seven_day as $weekly
    | select(
        ($weekly.used_percentage | type) == "number"
        and ($weekly.resets_at | type) == "number"
      )
    | ([([100 - $weekly.used_percentage, 0] | max), 100] | min) as $remaining
    | [($remaining * 10 | round / 10), ($remaining / 10 | floor), $weekly.resets_at]
    | @tsv
  '
)

# ANSI colors
GREEN='\033[32m'
YELLOW='\033[33m'
RED='\033[31m'
CYAN='\033[36m'
DIM='\033[2m'
RESET='\033[0m'
BOLD_BLUE='\033[1;34m'
BRIGHT_ORANGE='\033[1;33m'

# Shorten home directory to ~
home="$HOME"
short_cwd="${cwd/#$home/~}"

# Git branch (cached to avoid slowness in large repos)
CACHE_FILE="/tmp/claude-statusline-git-cache"
CACHE_MAX_AGE=5
git_branch=""

cache_is_stale() {
  [ ! -f "$CACHE_FILE" ] && return 0
  local file_age
  # Try GNU stat first, then BSD stat
  if file_age=$(stat -c %Y "$CACHE_FILE" 2>/dev/null); then
    [ $(($(date +%s) - file_age)) -gt $CACHE_MAX_AGE ]
  elif file_age=$(stat -f %m "$CACHE_FILE" 2>/dev/null); then
    [ $(($(date +%s) - file_age)) -gt $CACHE_MAX_AGE ]
  else
    return 0
  fi
}

if cache_is_stale; then
  if git -C "$cwd" rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    git_dir=$(git -C "$cwd" rev-parse --git-dir 2>/dev/null)
    git_common_dir=$(git -C "$cwd" rev-parse --git-common-dir 2>/dev/null)
    if [ "$git_dir" != "$git_common_dir" ]; then
      wt_name=$(basename "$(git -C "$cwd" rev-parse --show-toplevel 2>/dev/null)")
      git_branch="worktree(${wt_name})"
    else
      git_branch=$(git -C "$cwd" symbolic-ref --short HEAD 2>/dev/null || git -C "$cwd" rev-parse --short HEAD 2>/dev/null)
    fi
  fi
  echo "$git_branch" > "$CACHE_FILE"
else
  git_branch=$(cat "$CACHE_FILE")
fi

# Format token count (e.g., 15234 -> 15.2k, 1234567 -> 1.2M)
format_tokens() {
  local n=$1
  if [ "$n" -ge 1000000 ]; then
    printf '%s.%sM' "$((n / 1000000))" "$(( (n % 1000000) / 100000 ))"
  elif [ "$n" -ge 1000 ]; then
    printf '%s.%sk' "$((n / 1000))" "$(( (n % 1000) / 100 ))"
  else
    printf '%s' "$n"
  fi
}

# Section separator
SEP=" ${DIM}|${RESET} "

# Weekly Claude.ai subscription limit (remaining percentage and reset time)
weekly_info=""
weekly_plain=""
if [ -n "$weekly_remaining" ]; then
  WEEKLY_EMPTY=$((10 - weekly_filled))
  WEEKLY_BAR=""
  if [ "$weekly_filled" -gt 0 ]; then
    WEEKLY_BAR="${GREEN}$(printf "%${weekly_filled}s" | tr ' ' '#')${RESET}"
  fi
  if [ "$WEEKLY_EMPTY" -gt 0 ]; then
    WEEKLY_BAR="${WEEKLY_BAR}${DIM}$(printf "%${WEEKLY_EMPTY}s" | tr ' ' '-')${RESET}"
  fi

  if weekly_reset_at=$(date -r "$weekly_reset" '+%m/%d %H:%M' 2>/dev/null) ||
    weekly_reset_at=$(date -d "@$weekly_reset" '+%m/%d %H:%M' 2>/dev/null); then
    weekly_info="${WEEKLY_BAR} ${weekly_remaining}% ${DIM}⏳${weekly_reset_at}${RESET}"
    # The plain form tracks terminal cells; ⏳ occupies two.
    weekly_plain="$(printf "%${weekly_filled}s" | tr ' ' '#')$(printf "%${WEEKLY_EMPTY}s" | tr ' ' '-') ${weekly_remaining}%   ${weekly_reset_at}"
  fi
fi

# Context progress bar (colored = for filled, dim . for empty)
context_info=""
context_plain=""
if [ -n "$used" ]; then
  used_int=${used%.*}
  BAR_WIDTH=10
  FILLED=$((used_int * BAR_WIDTH / 100))
  EMPTY=$((BAR_WIDTH - FILLED))

  if [ "$used_int" -ge 90 ]; then BAR_COLOR="$RED"
  elif [ "$used_int" -ge 70 ]; then BAR_COLOR="$YELLOW"
  else BAR_COLOR="$GREEN"; fi

  BAR=""
  PLAIN_BAR=""
  if [ "$FILLED" -gt 0 ]; then
    FILLED_BAR=$(printf "%${FILLED}s" | tr ' ' '#')
    BAR="${BAR_COLOR}${FILLED_BAR}${RESET}"
    PLAIN_BAR="${FILLED_BAR}"
  fi
  if [ "$EMPTY" -gt 0 ]; then
    EMPTY_BAR=$(printf "%${EMPTY}s" | tr ' ' '-')
    BAR="${BAR}${DIM}${EMPTY_BAR}${RESET}"
    PLAIN_BAR="${PLAIN_BAR}${EMPTY_BAR}"
  fi

  context_info="${BAR} ${used_int}%"
  context_plain="${PLAIN_BAR} ${used_int}%"
fi

# Token usage (arrow indicators: input↓ output↑)
token_info=""
token_plain=""
if [ -n "$input_tokens" ] && [ -n "$output_tokens" ]; then
  formatted_input=$(format_tokens "$input_tokens")
  formatted_output=$(format_tokens "$output_tokens")
  token_info="${DIM}tokens${RESET} ${BOLD_BLUE}↓${formatted_input}${RESET} ${BRIGHT_ORANGE}↑${formatted_output}${RESET}"
  token_plain="tokens ↓${formatted_input} ↑${formatted_output}"
fi

# Lines changed
lines_info=""
lines_plain=""
if [ -n "$lines_added" ] && [ -n "$lines_removed" ]; then
  lines_info="${DIM}lines${RESET} ${GREEN}+${lines_added}${RESET} ${RED}-${lines_removed}${RESET}"
  lines_plain="lines +${lines_added} -${lines_removed}"
fi

# Compose left-aligned sections. Less important trailing sections are dropped
# first when the terminal is too narrow to preserve the right-aligned model.
styled_sections=("${CYAN}${short_cwd}${RESET}")
plain_sections=("${short_cwd}")

append_section() {
  local index=${#styled_sections[@]}
  styled_sections[$index]="$1"
  plain_sections[$index]="$2"
}

[ -n "$git_branch" ] && append_section "$git_branch" "$git_branch"
[ -n "$context_info" ] && append_section "$context_info" "$context_plain"
[ -n "$token_info" ] && append_section "$token_info" "$token_plain"
[ -n "$lines_info" ] && append_section "$lines_info" "$lines_plain"

section_count=${#styled_sections[@]}
compose_left() {
  left_output="${styled_sections[0]}"
  left_plain="${plain_sections[0]}"
  local i
  for ((i = 1; i < section_count; i++)); do
    left_output="${left_output}${SEP}${styled_sections[$i]}"
    left_plain="${left_plain} | ${plain_sections[$i]}"
  done
}
compose_left

model_output="$model"
model_plain="$model"
if [ -n "$effort" ]; then
  model_output="${model_output} ${DIM}·${RESET} ${BRIGHT_ORANGE}${effort}${RESET}"
  model_plain="${model_plain} · ${effort}"
fi

right_output="$model_output"
right_plain="$model_plain"
if [ -n "$weekly_info" ]; then
  right_output="${weekly_info}${SEP}${model_output}"
  right_plain="${weekly_plain} | ${model_plain}"
fi

# 터미널을 소유하지 않아 tput 대신 Claude Code의 COLUMNS를 사용한다.
# footer가 사용자 status line 바깥의 좌우 두 칸씩을 차지하므로 4를 뺀다.
columns=${COLUMNS:-0}
if [[ "$columns" =~ ^[0-9]+$ ]] && [ "$columns" -gt 0 ]; then
  columns=$((columns - 4))
  [ "$columns" -lt 1 ] && columns=1
  while [ "$section_count" -gt 1 ] &&
    [ $((${#left_plain} + ${#right_plain} + 2)) -gt "$columns" ]; do
    section_count=$((section_count - 1))
    compose_left
  done

  if [ "${#right_plain}" -ge "$columns" ] && [ -n "$weekly_info" ]; then
    right_output="$model_output"
    right_plain="$model_plain"
  fi

  if [ $((${#left_plain} + ${#right_plain} + 2)) -le "$columns" ]; then
    padding=$((columns - ${#left_plain} - ${#right_plain}))
    printf '%b%*s%b' "$left_output" "$padding" '' "$right_output"
  elif [ "${#right_plain}" -lt "$columns" ]; then
    padding=$((columns - ${#right_plain}))
    printf '%*s%b' "$padding" '' "$right_output"
  else
    printf '%b' "$right_output"
  fi
else
  printf '%b%b%b' "$left_output" "$SEP" "$right_output"
fi
