//! Reminders — natural language time, recurring, Windows Task Scheduler, dead drop.
//! Ported from reminder-rs, fully native.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;

use crate::config::get_config;

#[derive(Serialize, Deserialize, Clone)]
struct Reminder {
    id: String,
    text: String,
    due: String,
    created: String,
    completed: Option<String>,
    recurring: Option<String>,
    status: String,
    /// Occurrences of a recurrence that came and went with nobody completing them.
    #[serde(default)]
    missed_occurrences: u64,
    /// When the re-arm last caught this recurrence up, if it ever has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_rearmed: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ReminderStore {
    reminders: Vec<Reminder>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DeadDropMessage {
    id: String,
    date: String,
    message: String,
    from: String,
    priority: String,
    created: String,
    read: bool,
}

fn reminders_path() -> PathBuf {
    get_config().data_dir.join("reminders.json")
}

fn dead_drop_path() -> PathBuf {
    get_config().data_dir.join("dead_drop.json")
}

fn load_reminders() -> ReminderStore {
    let path = reminders_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(ReminderStore {
                reminders: Vec::new(),
            })
    } else {
        ReminderStore {
            reminders: Vec::new(),
        }
    }
}

fn save_reminders(store: &ReminderStore) -> Result<()> {
    let path = reminders_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

fn parse_due(input: &str) -> Result<DateTime<Utc>> {
    let s = input.trim().to_lowercase();
    let now = Local::now();

    // ISO8601 passthrough
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Relative: "in X hours/minutes/days"
    if s.starts_with("in ") {
        let parts: Vec<&str> = s
            .strip_prefix("in ")
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if parts.len() >= 2 {
            if let Ok(n) = parts[0].parse::<i64>() {
                let unit = parts[1];
                let dur = match unit {
                    u if u.starts_with("min") => Duration::minutes(n),
                    u if u.starts_with("hour") => Duration::hours(n),
                    u if u.starts_with("day") => Duration::days(n),
                    u if u.starts_with("week") => Duration::weeks(n),
                    _ => Duration::hours(n),
                };
                return Ok((now + dur).with_timezone(&Utc));
            }
        }
    }

    // "tomorrow" / "tomorrow at 9am"
    if s.starts_with("tomorrow") {
        let tomorrow = now + Duration::days(1);
        let time = extract_time_from_str(&s).unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let dt = tomorrow.date_naive().and_time(time);
        return Ok(dt.and_local_timezone(Local).unwrap().with_timezone(&Utc));
    }

    // "next monday/tuesday/etc"
    let weekdays = [
        ("monday", Weekday::Mon),
        ("tuesday", Weekday::Tue),
        ("wednesday", Weekday::Wed),
        ("thursday", Weekday::Thu),
        ("friday", Weekday::Fri),
        ("saturday", Weekday::Sat),
        ("sunday", Weekday::Sun),
    ];
    for (name, wd) in &weekdays {
        if s.contains(name) {
            let mut target = now + Duration::days(1);
            while target.weekday() != *wd {
                target += Duration::days(1);
            }
            let time =
                extract_time_from_str(&s).unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
            let dt = target.date_naive().and_time(time);
            return Ok(dt.and_local_timezone(Local).unwrap().with_timezone(&Utc));
        }
    }

    // Fallback: 1 hour from now
    Ok((now + Duration::hours(1)).with_timezone(&Utc))
}

fn extract_time_from_str(s: &str) -> Option<NaiveTime> {
    // Look for patterns like "9am", "9:30am", "9:30 pm", "14:00"
    let re = regex::Regex::new(r"(\d{1,2}):?(\d{2})?\s*(am|pm)?").ok()?;
    if let Some(caps) = re.captures(s) {
        let mut hour: u32 = caps.get(1)?.as_str().parse().ok()?;
        let min: u32 = caps
            .get(2)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        if let Some(ampm) = caps.get(3) {
            if ampm.as_str() == "pm" && hour < 12 {
                hour += 12;
            }
            if ampm.as_str() == "am" && hour == 12 {
                hour = 0;
            }
        }
        return NaiveTime::from_hms_opt(hour, min, 0);
    }
    None
}

pub async fn add_reminder(args: Value) -> Result<Value> {
    let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let due_str = args
        .get("due")
        .and_then(|v| v.as_str())
        .unwrap_or("in 1 hour");
    let recurring = args
        .get("recurring")
        .and_then(|v| v.as_str())
        .map(String::from);

    let due = parse_due(due_str)?;
    let reminder = Reminder {
        id: Uuid::new_v4().to_string()[..8].to_string(),
        text: text.to_string(),
        due: due.to_rfc3339(),
        created: Utc::now().to_rfc3339(),
        completed: None,
        recurring,
        status: "pending".to_string(),
        missed_occurrences: 0,
        last_rearmed: None,
    };

    let mut store = load_reminders();
    store.reminders.push(reminder.clone());
    save_reminders(&store)?;

    Ok(json!({
        "id": reminder.id,
        "text": reminder.text,
        "due": reminder.due,
        "recurring": reminder.recurring,
        "status": "created"
    }))
}

pub async fn list_reminders(args: Value) -> Result<Value> {
    let filter = args
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let store = load_reminders();
    let now = Utc::now();

    let filtered: Vec<&Reminder> = store
        .reminders
        .iter()
        .filter(|r| match filter {
            "pending" => r.status == "pending",
            "completed" => r.status == "completed",
            "overdue" => {
                r.status == "pending"
                    && DateTime::parse_from_rfc3339(&r.due)
                        .map(|d| d < now)
                        .unwrap_or(false)
            }
            "all" => true,
            _ => r.status == "pending",
        })
        .collect();

    let json_reminders: Vec<Value> = filtered.iter().map(|r| json!({
        "id": r.id, "text": r.text, "due": r.due, "status": r.status, "recurring": r.recurring
    })).collect();

    Ok(json!({ "reminders": json_reminders, "count": json_reminders.len() }))
}

pub async fn complete_reminder(args: Value) -> Result<Value> {
    let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let now = Utc::now();
    let mut store = load_reminders();

    let mut completed_text = String::new();
    let mut next_due = None;

    for r in &mut store.reminders {
        if r.id == id {
            r.status = "completed".to_string();
            r.completed = Some(now.to_rfc3339());
            completed_text = r.text.clone();

            // Handle recurring
            if let Some(ref schedule) = r.recurring {
                // The next occurrence must land strictly in the FUTURE: stepping one
                // interval off a stale due instant minted a replacement already overdue.
                let current_due = DateTime::parse_from_rfc3339(&r.due)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or(now);
                let (next, skipped) = advance_past(current_due, schedule, now);
                next_due = Some((r.text.clone(), next.to_rfc3339(), schedule.clone(), skipped));
            }
            break;
        }
    }

    // Create next occurrence if recurring
    if let Some((text, due, schedule, skipped)) = next_due {
        store.reminders.push(Reminder {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            text,
            due,
            created: now.to_rfc3339(),
            completed: None,
            recurring: Some(schedule),
            status: "pending".to_string(),
            missed_occurrences: skipped.saturating_sub(1),
            last_rearmed: None,
        });
    }

    save_reminders(&store)?;
    Ok(json!({ "status": "completed", "id": id, "text": completed_text }))
}

pub async fn delete_reminder(args: Value) -> Result<Value> {
    let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let mut store = load_reminders();
    store.reminders.retain(|r| r.id != id);
    save_reminders(&store)?;
    Ok(json!({ "status": "deleted", "id": id }))
}

pub async fn check_due(_args: Value) -> Result<Value> {
    let now = Utc::now();
    let (store, rearmed) = load_reminders_rearmed(now);

    let due: Vec<Value> = store
        .reminders
        .iter()
        .filter(|r| r.status == "pending")
        .filter(|r| {
            DateTime::parse_from_rfc3339(&r.due)
                .map(|d| d <= now)
                .unwrap_or(false)
        })
        .map(|r| json!({ "id": r.id, "text": r.text, "due": r.due }))
        .collect();

    Ok(json!({ "due": due, "count": due.len(), "rearmed": rearmed }))
}

pub async fn add_recurring(args: Value) -> Result<Value> {
    let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let schedule = args
        .get("schedule")
        .and_then(|v| v.as_str())
        .unwrap_or("daily");
    let time = args
        .get("time")
        .and_then(|v| v.as_str())
        .unwrap_or("9:00 AM");

    // DEFECT 6 (2026-09-03): the first occurrence used to be the literal string
    // "tomorrow at <time>", so every recurring reminder skipped its first real due
    // instant - a daily 09:00 reminder created at 08:00 did not fire until the NEXT
    // morning, and a weekly/monthly schedule got a first due that had nothing to do
    // with its own cadence. Compute a real first due from the schedule instead.
    let requested_time = extract_time_from_str(&time.to_lowercase()).ok_or_else(|| {
        anyhow!(
            "Unrecognized recurring time '{time}'. Use forms like '9:00 AM', '09:00', or '14:30'."
        )
    })?;
    let first_due = first_recurring_due(Local::now(), requested_time, schedule);

    let mut created = add_reminder(json!({
        "text": text,
        "due": first_due.with_timezone(&Utc).to_rfc3339(),
        "recurring": schedule
    }))
    .await?;

    if let Some(obj) = created.as_object_mut() {
        obj.insert("schedule".to_string(), json!(schedule));
        obj.insert("first_due_local".to_string(), json!(first_due.to_rfc3339()));
    }
    Ok(created)
}

/// First real occurrence of a recurring reminder: today at the requested time when that
/// instant is still ahead, otherwise the next instant the schedule itself defines.
fn first_recurring_due(now: DateTime<Local>, time: NaiveTime, schedule: &str) -> DateTime<Local> {
    let today = local_at(now.date_naive(), time, now);
    if today > now {
        return today;
    }
    today + schedule_step(schedule)
}

/// Resolve a naive local date+time, tolerating DST gaps and folds rather than panicking.
fn local_at(date: NaiveDate, time: NaiveTime, fallback: DateTime<Local>) -> DateTime<Local> {
    let naive = date.and_time(time);
    match naive.and_local_timezone(Local) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(earliest, _) => earliest,
        // Spring-forward gap: this wall-clock time does not exist. Step past the gap.
        chrono::LocalResult::None => (naive + Duration::hours(1))
            .and_local_timezone(Local)
            .earliest()
            .unwrap_or(fallback),
    }
}

fn schedule_step(schedule: &str) -> Duration {
    match schedule {
        "hourly" => Duration::hours(1),
        "weekly" => Duration::weeks(1),
        "monthly" => Duration::days(30),
        _ => Duration::days(1),
    }
}

/// Roll a recurrence forward until it lands strictly after `after`.
/// Returns the new instant and how many whole occurrences were stepped over.
fn advance_past(from: DateTime<Utc>, schedule: &str, after: DateTime<Utc>) -> (DateTime<Utc>, u64) {
    let step = schedule_step(schedule);
    if step <= Duration::zero() || from > after {
        return (from, 0);
    }
    let gap = after.signed_duration_since(from);
    // Whole steps needed to clear `after`, then one more so the result is strictly later.
    let steps = (gap.num_seconds() / step.num_seconds()) + 1;
    (from + step * (steps as i32), steps as u64)
}

/// Latest occurrence at or before `at` - the catch-up point for a recurrence nobody
/// ever completed. Keeps the reminder DUE instead of hiding it in the future.
fn catch_up_to(from: DateTime<Utc>, schedule: &str, at: DateTime<Utc>) -> (DateTime<Utc>, u64) {
    let step = schedule_step(schedule);
    if step <= Duration::zero() || from >= at {
        return (from, 0);
    }
    let gap = at.signed_duration_since(from);
    let steps = gap.num_seconds() / step.num_seconds();
    (from + step * (steps as i32), steps as u64)
}

/// DEFECT 7 (2026-09-03): a recurring reminder only ever advanced inside
/// `complete_reminder`. One that was never completed stayed pinned to its original due
/// instant forever, and `complete_reminder` then computed the next occurrence from that
/// stale instant, creating an occurrence already in the past.
///
/// Re-arm on read: catch a stale recurrence up to its most recent occurrence at or
/// before now, so it still surfaces as due (it genuinely is) but stops accumulating
/// dead backlog. The number of skipped occurrences is preserved, not discarded.
fn rearm_recurring(store: &mut ReminderStore, now: DateTime<Utc>) -> u64 {
    let mut rearmed = 0u64;
    for reminder in &mut store.reminders {
        if reminder.status != "pending" {
            continue;
        }
        let Some(schedule) = reminder.recurring.clone() else {
            continue;
        };
        let Ok(due) = DateTime::parse_from_rfc3339(&reminder.due) else {
            continue;
        };
        let (caught_up, missed) = catch_up_to(due.with_timezone(&Utc), &schedule, now);
        if missed > 0 {
            reminder.due = caught_up.to_rfc3339();
            reminder.missed_occurrences += missed;
            reminder.last_rearmed = Some(now.to_rfc3339());
            rearmed += 1;
        }
    }
    rearmed
}

/// Re-arm stale recurrences and persist only when something actually moved.
fn load_reminders_rearmed(now: DateTime<Utc>) -> (ReminderStore, u64) {
    let mut store = load_reminders();
    let rearmed = rearm_recurring(&mut store, now);
    if rearmed > 0 {
        // A failed re-arm write must not fail the read; the caller still gets correct
        // due-ness from the in-memory catch-up, it just is not persisted yet.
        let _ = save_reminders(&store);
    }
    (store, rearmed)
}

// ---------------------------------------------------------------------------
// Windows Task Scheduler reminders
//
// PATCH SPEC 2026-08-28 "scheduled_reminders_silently_broken.md", five defects.
// The spec observed that autonomous.exe and ops.exe carry IDENTICAL code differing
// only in the CPC_/Ops_ task prefix, and that it is therefore one fix. This is the
// ops half of it, ported from the autonomous fix shipped as P008 on 2026-09-03.
// Together the defects meant the tool returned a receipt-shaped success and created
// nothing: `task_name` and `time` were the caller's own arguments echoed back, and
// `output` was empty because the schtasks invocation had failed and its exit code
// was discarded.
//
// Everything here is pure and unit-tested; no test invokes schtasks.
// ---------------------------------------------------------------------------

/// schtasks refuses a `/TR` value longer than this.
const SCHTASKS_TR_LIMIT: usize = 261;
/// The action wrapper every reminder task runs; its length is charged against the limit.
const SCHTASKS_MSG_PREFIX: &str = "msg * /TIME:10 ";
/// Task-name prefix that marks a task as one of ours. This is the ONLY difference
/// between this implementation and the autonomous crate's.
const SCHEDULED_PREFIX: &str = "Ops_";

/// Usable message budget once the action wrapper is charged (261 - 15 = 246).
fn scheduled_message_budget() -> usize {
    SCHTASKS_TR_LIMIT - SCHTASKS_MSG_PREFIX.len()
}

/// Order of the day/month/year fields in the running account's short-date pattern.
/// DEFECT 2, second half: `/SD` is locale-formatted, so a hardcoded `MM/DD/YYYY` is a
/// latent bug on any machine set to `DD/MM/YYYY`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateOrder {
    Mdy,
    Dmy,
    Ymd,
}

fn parse_short_date_order(pattern: &str) -> DateOrder {
    for ch in pattern.chars() {
        match ch.to_ascii_lowercase() {
            'd' => return DateOrder::Dmy,
            'm' => return DateOrder::Mdy,
            'y' => return DateOrder::Ymd,
            _ => {}
        }
    }
    DateOrder::Mdy
}

fn format_start_date(date: NaiveDate, order: DateOrder) -> String {
    let (d, m, y) = (date.day(), date.month(), date.year());
    match order {
        DateOrder::Mdy => format!("{m:02}/{d:02}/{y:04}"),
        DateOrder::Dmy => format!("{d:02}/{m:02}/{y:04}"),
        DateOrder::Ymd => format!("{y:04}/{m:02}/{d:02}"),
    }
}

/// Read the account short-date pattern once per process.
fn detected_date_order() -> DateOrder {
    static ORDER: once_cell::sync::Lazy<DateOrder> = once_cell::sync::Lazy::new(|| {
        std::process::Command::new("reg")
            .args([
                "query",
                "HKCU\\Control Panel\\International",
                "/v",
                "sShortDate",
            ])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .find(|line| line.contains("sShortDate"))
                    .and_then(|line| line.split_whitespace().last().map(str::to_string))
            })
            .map(|pattern| parse_short_date_order(&pattern))
            .unwrap_or(DateOrder::Mdy)
    });
    *ORDER
}

/// A caller-supplied scheduled time, resolved to a real local instant.
#[derive(Debug, Clone, PartialEq)]
struct ScheduledWhen {
    at: DateTime<Local>,
    /// How the input was read, echoed back so a caller can see what "07:30" became.
    interpretation: String,
}

/// DEFECT 1: the ISO timestamp the tool's own signature invites was passed straight to
/// `/ST`, which takes `HH:mm` only and rejected it - so no reminder carrying an ISO
/// timestamp was ever created. Parse permissively here, and resolve a real date too.
///
/// Documented rule for a bare `HH:mm`: it means TODAY when that instant is still ahead,
/// otherwise TOMORROW. It never yields a past instant, because "today at a time that
/// already passed" is exactly what silently produced dead tasks under defect 2.
fn parse_scheduled_time(input: &str, now: DateTime<Local>) -> Result<ScheduledWhen> {
    let s = input.trim();
    if s.is_empty() {
        return Err(anyhow!(
            "Scheduled reminder needs a time. Accepted: '2026-08-29T07:30:00', '2026-08-29 07:30', '2026-08-29', or '07:30'."
        ));
    }

    let resolved: Option<(DateTime<Local>, String)> = None
        .or_else(|| {
            DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| (dt.with_timezone(&Local), "ISO 8601 with offset".to_string()))
        })
        .or_else(|| {
            for fmt in [
                "%Y-%m-%dT%H:%M:%S",
                "%Y-%m-%dT%H:%M",
                "%Y-%m-%d %H:%M:%S",
                "%Y-%m-%d %H:%M",
                "%Y/%m/%d %H:%M",
            ] {
                if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
                    return Some((
                        local_at(naive.date(), naive.time(), now),
                        "local date and time".to_string(),
                    ));
                }
            }
            None
        })
        .or_else(|| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().map(|date| {
                (
                    local_at(date, NaiveTime::from_hms_opt(9, 0, 0).unwrap(), now),
                    "date only, defaulted to 09:00 local".to_string(),
                )
            })
        })
        .or_else(|| {
            for fmt in ["%H:%M:%S", "%H:%M"] {
                if let Ok(time) = NaiveTime::parse_from_str(s, fmt) {
                    let today = local_at(now.date_naive(), time, now);
                    return Some(if today > now {
                        (today, "bare time, today".to_string())
                    } else {
                        (
                            local_at(now.date_naive() + Duration::days(1), time, now),
                            "bare time already passed today, rolled to tomorrow".to_string(),
                        )
                    });
                }
            }
            None
        });

    let (at, interpretation) = resolved.ok_or_else(|| {
        anyhow!(
            "Unrecognized time '{s}'. Accepted: '2026-08-29T07:30:00', '2026-08-29 07:30', '2026-08-29', or '07:30'."
        )
    })?;

    // A task whose start instant has already passed is created by schtasks with exit 0
    // and an EMPTY NextRunTime - it exists and can never fire. Refuse instead.
    if at <= now {
        return Err(anyhow!(
            "Refusing to schedule '{s}' - it resolves to {} which is already past (now {}). A past start time produces a task that exists and never fires.",
            at.to_rfc3339(),
            now.to_rfc3339()
        ));
    }

    Ok(ScheduledWhen { at, interpretation })
}

/// Make a message safe for a quoted `cmd` argument and bound it to the `/TR` budget.
/// DEFECT 3: the message was unbounded, so any multi-sentence reminder blew the
/// 261-character `/TR` ceiling and the failure was reported as success.
fn prepare_scheduled_message(message: &str) -> (String, Option<String>) {
    let sanitized: String = message
        .replace('"', "'")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let budget = scheduled_message_budget();
    let count = sanitized.chars().count();
    if count <= budget {
        return (sanitized, None);
    }
    let truncated: String = sanitized.chars().take(budget).collect();
    let prefix_len = SCHTASKS_MSG_PREFIX.len();
    (
        truncated,
        Some(format!(
            "Message truncated from {count} to {budget} characters: schtasks rejects a /TR value longer than {SCHTASKS_TR_LIMIT} characters and the action wrapper consumes {prefix_len}."
        )),
    )
}

fn scheduled_task_name(name: &str) -> String {
    let trimmed = name.trim();
    let base = if trimmed.is_empty() {
        "Reminder"
    } else {
        trimmed
    };
    if base.starts_with(SCHEDULED_PREFIX) {
        base.replace(' ', "_")
    } else {
        format!("{SCHEDULED_PREFIX}{}", base.replace(' ', "_"))
    }
}

/// The construction PAL verified: `/SC ONCE` plus BOTH `/SD` and `/ST`.
/// DEFECT 2: without `/SD` a reminder could only ever be for today.
fn build_create_command(
    task_name: &str,
    message: &str,
    when: &ScheduledWhen,
    order: DateOrder,
) -> String {
    format!(
        "schtasks /Create /TN \"{}\" /TR \"{}{}\" /SC ONCE /SD \"{}\" /ST \"{}\" /F",
        task_name,
        SCHTASKS_MSG_PREFIX,
        message,
        format_start_date(when.at.date_naive(), order),
        when.at.format("%H:%M")
    )
}

/// Pull one labelled field out of `schtasks /Query /FO LIST /V` output.
fn extract_list_field(list_output: &str, label: &str) -> Option<String> {
    let needle = label.to_lowercase();
    list_output
        .lines()
        .find(|line| line.trim_start().to_lowercase().starts_with(&needle))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Split one CSV record, honouring doubled quotes inside quoted fields.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// DEFECT 5: `schtasks /Query /TN` takes an EXACT task name and does not accept
/// wildcards, so `/TN \Ops_*` always exited 1 and `list_scheduled` always returned
/// `{"tasks":""}` - read by every caller as "zero reminders exist" when it actually
/// meant "this query cannot match anything". Enumerate everything and filter here.
fn parse_scheduled_csv(csv: &str, prefix: &str) -> Vec<Value> {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let Some(header_line) = lines.next() else {
        return Vec::new();
    };
    let header = split_csv_line(header_line);
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let name_idx = column("TaskName");
    let next_idx = column("Next Run Time");
    let status_idx = column("Status");
    let run_idx = column("Task To Run");

    let mut tasks = Vec::new();
    for line in lines {
        // schtasks repeats the header row once per folder it enumerates.
        if line == header_line {
            continue;
        }
        let fields = split_csv_line(line);
        let get = |idx: Option<usize>| {
            idx.and_then(|i| fields.get(i))
                .map(|v| v.trim().to_string())
                .unwrap_or_default()
        };
        let full_name = get(name_idx);
        // TaskName arrives as a path such as \Ops_Standup; match on the leaf.
        let leaf = full_name
            .rsplit('\\')
            .next()
            .unwrap_or(full_name.as_str())
            .to_string();
        if !leaf.starts_with(prefix) {
            continue;
        }
        tasks.push(json!({
            "task_name": leaf,
            "path": full_name,
            "next_run_time": get(next_idx),
            "status": get(status_idx),
            "task_to_run": get(run_idx),
        }));
    }
    tasks
}

/// Outcome of a `cmd /C` invocation with the exit code and BOTH streams kept.
/// DEFECT 4: the exit code was discarded and stdout was captured but never inspected,
/// so defects 1 and 3 - which both exit non-zero - were reported as success.
struct ShellResult {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_cmd(command: &str) -> Result<ShellResult> {
    let output = std::process::Command::new("cmd")
        .args(["/C", command])
        .output()
        .with_context(|| format!("failed to spawn: {command}"))?;
    Ok(ShellResult {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// True when a queried task has no usable next run time, i.e. it exists and cannot fire.
fn scheduled_task_is_dead(next_run_time: Option<&str>) -> bool {
    match next_run_time {
        None => true,
        Some(value) => {
            let v = value.trim();
            v.is_empty() || v.eq_ignore_ascii_case("N/A")
        }
    }
}

pub async fn add_scheduled(args: Value) -> Result<Value> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Reminder");
    let time = args.get("time").and_then(|v| v.as_str()).unwrap_or("09:00");
    let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");

    let task_name = scheduled_task_name(name);
    let when = parse_scheduled_time(time, Local::now())?;
    let (safe_message, truncation) = prepare_scheduled_message(message);
    let command = build_create_command(&task_name, &safe_message, &when, detected_date_order());

    let created = run_cmd(&command)?;
    if created.code != 0 {
        return Err(anyhow!(
            "schtasks failed to create '{}' (exit {}): {} {}\nCommand: {}",
            task_name,
            created.code,
            created.stderr,
            created.stdout,
            command
        ));
    }

    // Verify by reading the task back. schtasks can exit 0 with only a WARNING on stdout
    // and still leave a task with an empty NextRunTime; this single check catches that,
    // and would have caught defects 1, 2 and 3 at the source.
    let verify = run_cmd(&format!("schtasks /Query /TN \"{task_name}\" /FO LIST /V"))?;
    let next_run_time = if verify.code == 0 {
        extract_list_field(&verify.stdout, "Next Run Time")
    } else {
        None
    };
    if scheduled_task_is_dead(next_run_time.as_deref()) {
        return Err(anyhow!(
            "schtasks reported success for '{}' but the task has no next run time, so it can never fire. Requested {} ({}). create said: {} | verify exit {}: {}",
            task_name,
            when.at.to_rfc3339(),
            when.interpretation,
            created.stdout,
            verify.code,
            verify.stdout
        ));
    }

    let mut result = json!({
        "status": "created",
        "task_name": task_name,
        "requested_time": time,
        "interpreted_as": when.interpretation,
        "scheduled_local": when.at.to_rfc3339(),
        "next_run_time": next_run_time,
        "exit_code": created.code,
        "output": created.stdout,
    });
    if let (Some(obj), Some(note)) = (result.as_object_mut(), truncation) {
        obj.insert("message_truncated".to_string(), json!(note));
    }
    Ok(result)
}

pub async fn list_scheduled(_args: Value) -> Result<Value> {
    let query = run_cmd("schtasks /Query /FO CSV /V")?;
    if query.code != 0 {
        return Err(anyhow!(
            "schtasks query failed (exit {}): {} {}",
            query.code,
            query.stderr,
            query.stdout
        ));
    }
    let tasks = parse_scheduled_csv(&query.stdout, SCHEDULED_PREFIX);
    Ok(json!({
        "tasks": tasks,
        "count": tasks.len(),
        "prefix": SCHEDULED_PREFIX,
    }))
}

pub async fn delete_scheduled(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if name.trim().is_empty() {
        return Err(anyhow!("delete_scheduled needs a task name."));
    }
    let task_name = scheduled_task_name(name);
    let deleted = run_cmd(&format!("schtasks /Delete /TN \"{task_name}\" /F"))?;
    if deleted.code != 0 {
        return Err(anyhow!(
            "schtasks failed to delete '{}' (exit {}): {} {}",
            task_name,
            deleted.code,
            deleted.stderr,
            deleted.stdout
        ));
    }
    Ok(json!({
        "status": "deleted",
        "task_name": task_name,
        "exit_code": deleted.code,
        "output": deleted.stdout,
    }))
}

pub async fn time_check(_args: Value) -> Result<Value> {
    let due = check_due(json!({})).await?;
    let count = due.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    Ok(json!({
        "current_time": Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "overdue_count": count,
        "reminders": due.get("due")
    }))
}

// === Dead Drop ===

fn load_dead_drop() -> Vec<DeadDropMessage> {
    let path = dead_drop_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_dead_drop(msgs: &[DeadDropMessage]) -> Result<()> {
    let path = dead_drop_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(msgs)?)?;
    Ok(())
}

pub async fn dead_drop_leave(args: Value) -> Result<Value> {
    let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let from = args
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or("autonomous");
    let priority = args
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    let msg = DeadDropMessage {
        id: Uuid::new_v4().to_string()[..8].to_string(),
        date: Local::now().format("%Y-%m-%d").to_string(),
        message: message.to_string(),
        from: from.to_string(),
        priority: priority.to_string(),
        created: Utc::now().to_rfc3339(),
        read: false,
    };

    let mut msgs = load_dead_drop();
    msgs.push(msg.clone());
    save_dead_drop(&msgs)?;

    Ok(json!({ "status": "left", "id": msg.id }))
}

pub async fn dead_drop_check(_args: Value) -> Result<Value> {
    let msgs = load_dead_drop();
    let unread: Vec<Value> = msgs.iter()
        .filter(|m| !m.read)
        .map(|m| json!({ "id": m.id, "message": m.message, "from": m.from, "priority": m.priority, "date": m.date }))
        .collect();
    Ok(json!({ "messages": unread, "count": unread.len() }))
}

pub async fn dead_drop_clear(args: Value) -> Result<Value> {
    let specific_id = args.get("id").and_then(|v| v.as_str());
    let mut msgs = load_dead_drop();

    match specific_id {
        Some(id) => {
            for m in &mut msgs {
                if m.id == id {
                    m.read = true;
                }
            }
        }
        None => {
            for m in &mut msgs {
                m.read = true;
            }
        }
    }

    save_dead_drop(&msgs)?;
    Ok(json!({ "status": "cleared" }))
}

/// Tests for the 2026-08-28 patch spec "scheduled reminders silently broken" (defects 1-5)
/// and the two recurrence defects found alongside it (6-7).
/// This is the ops half; the autonomous half shipped as P008 on 2026-09-03.
///
/// Every test here is PURE. The routing packet forbids enabling or invoking a scheduled
/// task as a test, so nothing below shells out to schtasks; the command construction,
/// time parsing, message bounding and query parsing are all exercised directly.
#[cfg(test)]
mod scheduled_reminder_tests {
    use super::*;

    fn local(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> DateTime<Local> {
        local_at(
            NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            NaiveTime::from_hms_opt(hh, mm, 0).unwrap(),
            Local::now(),
        )
    }

    fn utc(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(hh, mm, 0)
            .unwrap()
            .and_utc()
    }

    fn when_at(at: DateTime<Local>) -> ScheduledWhen {
        ScheduledWhen {
            at,
            interpretation: "test".to_string(),
        }
    }

    // -- DEFECT 1: the ISO timestamp reached /ST unparsed and schtasks rejected it -------

    #[test]
    fn defect1_iso_timestamp_is_parsed_instead_of_reaching_st_raw() {
        let now = local(2026, 8, 28, 17, 26);
        let when = parse_scheduled_time("2026-08-29T07:30:00", now)
            .expect("the signature's own ISO form must parse");
        assert_eq!(
            when.at.date_naive(),
            NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()
        );
        assert_eq!(when.at.format("%H:%M").to_string(), "07:30");

        // The whole point: /ST receives HH:mm, never the ISO string.
        let command = build_create_command("Ops_ZZISO", "test", &when, DateOrder::Mdy);
        assert!(
            command.contains("/ST \"07:30\""),
            "/ST must carry HH:mm only: {command}"
        );
        assert!(
            !command.contains("2026-08-29T07:30:00"),
            "the raw ISO string must never reach the command line: {command}"
        );
    }

    #[test]
    fn defect1_accepts_the_documented_input_forms() {
        let now = local(2026, 8, 28, 17, 26);
        for input in [
            "2026-08-29T07:30:00",
            "2026-08-29T07:30",
            "2026-08-29 07:30",
            "2026-08-29 07:30:00",
            "2026-08-29",
        ] {
            let when = parse_scheduled_time(input, now)
                .unwrap_or_else(|e| panic!("'{input}' should parse: {e}"));
            assert_eq!(
                when.at.date_naive(),
                NaiveDate::from_ymd_opt(2026, 8, 29).unwrap(),
                "'{input}' resolved to the wrong date"
            );
        }
    }

    #[test]
    fn defect1_rfc3339_with_offset_is_honoured() {
        let now = local(2026, 8, 28, 17, 26);
        let when = parse_scheduled_time("2026-08-29T07:30:00-04:00", now).expect("offset form");
        assert_eq!(when.interpretation, "ISO 8601 with offset");
        assert_eq!(
            when.at.to_utc(),
            utc(2026, 8, 29, 11, 30),
            "the offset must be respected, not stripped"
        );
    }

    #[test]
    fn defect1_garbage_and_empty_input_are_rejected_with_the_accepted_forms() {
        let now = local(2026, 8, 28, 17, 26);
        for bad in ["", "   ", "next tuesday-ish", "25:99"] {
            let err = parse_scheduled_time(bad, now)
                .expect_err("'{bad}' must not silently become a time")
                .to_string();
            assert!(
                err.contains("2026-08-29T07:30:00"),
                "the error should show the accepted forms: {err}"
            );
        }
    }

    // -- DEFECT 2: no /SD, so a reminder could only ever be for today -------------------

    #[test]
    fn defect2_create_command_carries_a_start_date() {
        let when = when_at(local(2026, 8, 29, 7, 30));
        let command = build_create_command("Ops_ZZCORRECT", "test", &when, DateOrder::Mdy);
        // This is the exact construction PAL verified with exit 0 and a correct NextRunTime.
        assert_eq!(
            command,
            "schtasks /Create /TN \"Ops_ZZCORRECT\" /TR \"msg * /TIME:10 test\" /SC ONCE /SD \"08/29/2026\" /ST \"07:30\" /F"
        );
    }

    #[test]
    fn defect2_bare_time_already_past_rolls_to_tomorrow_never_to_a_dead_task() {
        // The spec's reproduction: local time 17:26, caller asks for 07:30.
        let now = local(2026, 8, 28, 17, 26);
        let when = parse_scheduled_time("07:30", now).expect("bare time must parse");
        assert_eq!(
            when.at.date_naive(),
            NaiveDate::from_ymd_opt(2026, 8, 29).unwrap(),
            "a bare time that already passed must roll to tomorrow, not create a task for today"
        );
        assert!(when.at > now, "the resolved instant must be in the future");
        assert!(
            when.interpretation.contains("rolled to tomorrow"),
            "the interpretation must be reported back: {}",
            when.interpretation
        );
    }

    #[test]
    fn defect2_bare_time_still_ahead_means_today() {
        let now = local(2026, 8, 28, 6, 0);
        let when = parse_scheduled_time("07:30", now).expect("bare time must parse");
        assert_eq!(
            when.at.date_naive(),
            NaiveDate::from_ymd_opt(2026, 8, 28).unwrap()
        );
        assert_eq!(when.interpretation, "bare time, today");
    }

    #[test]
    fn defect2_a_past_instant_is_an_explicit_error_not_a_task_that_cannot_fire() {
        let now = local(2026, 8, 28, 17, 26);
        let err = parse_scheduled_time("2026-08-27T09:00", now)
            .expect_err("a past instant must be refused")
            .to_string();
        assert!(err.contains("already past"), "{err}");
        assert!(
            err.contains("never fires"),
            "the error should say why a past start time is refused: {err}"
        );
    }

    #[test]
    fn defect2_start_date_follows_the_account_locale_not_a_hardcoded_pattern() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 29).unwrap();
        assert_eq!(format_start_date(date, DateOrder::Mdy), "08/29/2026");
        assert_eq!(format_start_date(date, DateOrder::Dmy), "29/08/2026");
        assert_eq!(format_start_date(date, DateOrder::Ymd), "2026/08/29");
    }

    #[test]
    fn defect2_short_date_patterns_map_to_the_right_field_order() {
        assert_eq!(parse_short_date_order("M/d/yyyy"), DateOrder::Mdy);
        assert_eq!(parse_short_date_order("MM/dd/yyyy"), DateOrder::Mdy);
        assert_eq!(parse_short_date_order("dd/MM/yyyy"), DateOrder::Dmy);
        assert_eq!(parse_short_date_order("d.M.yyyy"), DateOrder::Dmy);
        assert_eq!(parse_short_date_order("yyyy-MM-dd"), DateOrder::Ymd);
        // Unreadable pattern falls back rather than panicking.
        assert_eq!(parse_short_date_order(""), DateOrder::Mdy);
    }

    // -- DEFECT 3: /TR is capped at 261 characters and the message was unbounded ---------

    #[test]
    fn defect3_message_budget_matches_the_measured_ceiling() {
        assert_eq!(SCHTASKS_MSG_PREFIX.len(), 15);
        assert_eq!(scheduled_message_budget(), 246);
        assert_eq!(
            scheduled_message_budget() + SCHTASKS_MSG_PREFIX.len(),
            SCHTASKS_TR_LIMIT
        );
    }

    #[test]
    fn defect3_oversized_message_is_truncated_and_reported_never_silently_dropped() {
        // The spec's reproduction: a 722-character message exited non-zero.
        let message = "x".repeat(722);
        let (prepared, note) = prepare_scheduled_message(&message);
        assert_eq!(prepared.chars().count(), 246);
        let note = note.expect("truncation must be reported to the caller");
        assert!(note.contains("722"), "{note}");
        assert!(note.contains("246"), "{note}");

        // And the resulting /TR value must actually fit.
        let when = when_at(local(2026, 8, 29, 7, 30));
        let command = build_create_command("Ops_ZZLONG", &prepared, &when, DateOrder::Mdy);
        let tr_value = command
            .split("/TR \"")
            .nth(1)
            .and_then(|rest| rest.split("\" /SC").next())
            .expect("command must contain a /TR value");
        assert!(
            tr_value.chars().count() <= SCHTASKS_TR_LIMIT,
            "/TR value is {} chars, over the {SCHTASKS_TR_LIMIT} limit",
            tr_value.chars().count()
        );
    }

    #[test]
    fn defect3_message_within_budget_is_untouched_and_unreported() {
        let (prepared, note) = prepare_scheduled_message("stand-up in five minutes");
        assert_eq!(prepared, "stand-up in five minutes");
        assert!(note.is_none(), "no truncation note when nothing was cut");
    }

    #[test]
    fn defect3_quotes_and_newlines_cannot_break_out_of_the_quoted_argument() {
        let (prepared, _) = prepare_scheduled_message("say \"hi\"\nthen\tstop");
        assert!(
            !prepared.contains('"'),
            "quotes must not survive: {prepared}"
        );
        assert!(
            !prepared.contains('\n'),
            "newlines must not survive: {prepared}"
        );
        assert!(
            !prepared.contains('\t'),
            "tabs must not survive: {prepared}"
        );
        assert_eq!(prepared, "say 'hi' then stop");
    }

    #[test]
    fn defect3_truncation_is_char_safe_for_multibyte_messages() {
        let message = "\u{e9}".repeat(400);
        let (prepared, note) = prepare_scheduled_message(&message);
        assert_eq!(prepared.chars().count(), 246);
        assert!(note.is_some());
    }

    // -- DEFECT 4: the exit code was discarded --------------------------------------------

    #[test]
    fn defect4_a_task_with_no_next_run_time_counts_as_dead() {
        // schtasks can exit 0 with only a WARNING and leave a task that never fires.
        assert!(scheduled_task_is_dead(None));
        assert!(scheduled_task_is_dead(Some("")));
        assert!(scheduled_task_is_dead(Some("   ")));
        assert!(scheduled_task_is_dead(Some("N/A")));
        assert!(scheduled_task_is_dead(Some("n/a")));
        assert!(!scheduled_task_is_dead(Some("08/29/2026 07:30:00")));
    }

    #[test]
    fn defect4_next_run_time_is_read_back_out_of_the_verify_query() {
        let list_output = "\
Folder: \\
HostName:                             PALADIN
TaskName:                             \\Ops_ZZCORRECT
Next Run Time:                        08/29/2026 07:30:00
Status:                               Ready
Task To Run:                          msg * /TIME:10 test";
        assert_eq!(
            extract_list_field(list_output, "Next Run Time").as_deref(),
            Some("08/29/2026 07:30:00")
        );
        assert_eq!(
            extract_list_field(list_output, "Status").as_deref(),
            Some("Ready")
        );
        // An empty value must read as absent, so the dead-task check fires.
        let empty = "TaskName:  \\Ops_ZZDEAD\nNext Run Time:\nStatus:  Ready";
        assert!(extract_list_field(empty, "Next Run Time").is_none());
        assert!(extract_list_field(list_output, "No Such Field").is_none());
    }

    // -- DEFECT 5: /TN takes an exact name, so the wildcard query never matched -----------

    #[test]
    fn defect5_csv_enumeration_finds_prefixed_tasks_the_wildcard_query_never_could() {
        let csv = "\
\"HostName\",\"TaskName\",\"Next Run Time\",\"Status\",\"Task To Run\"
\"PALADIN\",\"\\Ops_Standup\",\"08/29/2026 07:30:00\",\"Ready\",\"msg * /TIME:10 stand-up\"
\"PALADIN\",\"\\Ops_Review\",\"08/30/2026 09:00:00\",\"Ready\",\"msg * /TIME:10 review\"
\"PALADIN\",\"\\OneDrive Reporting Task\",\"N/A\",\"Ready\",\"onedrive.exe\"";
        let tasks = parse_scheduled_csv(csv, "Ops_");
        assert_eq!(tasks.len(), 2, "both CPC_ tasks must be returned");
        assert_eq!(tasks[0]["task_name"], "Ops_Standup");
        assert_eq!(tasks[0]["path"], "\\Ops_Standup");
        assert_eq!(tasks[0]["next_run_time"], "08/29/2026 07:30:00");
        assert_eq!(tasks[0]["status"], "Ready");
        assert_eq!(tasks[1]["task_name"], "Ops_Review");
    }

    #[test]
    fn defect5_foreign_tasks_and_repeated_headers_are_filtered_out() {
        let csv = "\
\"HostName\",\"TaskName\",\"Next Run Time\",\"Status\",\"Task To Run\"
\"PALADIN\",\"\\Microsoft\\Windows\\Defrag\\ScheduledDefrag\",\"N/A\",\"Ready\",\"defrag.exe\"
\"HostName\",\"TaskName\",\"Next Run Time\",\"Status\",\"Task To Run\"
\"PALADIN\",\"\\CPC_Reminder\",\"09/01/2026 09:00:00\",\"Ready\",\"msg * /TIME:10 ops\"";
        // schtasks repeats the header once per folder; it must not be parsed as a task.
        assert!(parse_scheduled_csv(csv, "Ops_").is_empty());
        // And the same enumeration serves the autonomous crate's prefix without a second query.
        let other = parse_scheduled_csv(csv, "CPC_");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0]["task_name"], "CPC_Reminder");
    }

    #[test]
    fn defect5_empty_or_headerless_output_is_zero_tasks_not_a_panic() {
        assert!(parse_scheduled_csv("", "Ops_").is_empty());
        assert!(parse_scheduled_csv("   \n  \n", "Ops_").is_empty());
        assert!(parse_scheduled_csv(
            "\"HostName\",\"TaskName\",\"Next Run Time\",\"Status\",\"Task To Run\"",
            "Ops_"
        )
        .is_empty());
    }

    #[test]
    fn defect5_quoted_commas_do_not_split_a_field() {
        let csv = "\
\"HostName\",\"TaskName\",\"Next Run Time\",\"Status\",\"Task To Run\"
\"PALADIN\",\"\\Ops_Comma\",\"08/29/2026 07:30:00\",\"Ready\",\"msg * /TIME:10 do a, then b\"";
        let tasks = parse_scheduled_csv(csv, "Ops_");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["task_to_run"], "msg * /TIME:10 do a, then b");
        assert_eq!(tasks[0]["next_run_time"], "08/29/2026 07:30:00");
    }

    #[test]
    fn defect5_task_name_normalisation_is_stable_between_create_and_delete() {
        // add_scheduled and delete_scheduled must agree on the resolved task name,
        // otherwise a reminder cannot be deleted by the name it was created with.
        assert_eq!(scheduled_task_name("Standup"), "Ops_Standup");
        assert_eq!(scheduled_task_name("morning review"), "Ops_morning_review");
        assert_eq!(scheduled_task_name("Ops_Standup"), "Ops_Standup");
        assert_eq!(scheduled_task_name("  "), "Ops_Reminder");
        assert_eq!(scheduled_task_name(""), "Ops_Reminder");
    }

    // -- DEFECT 6: add_recurring never computed a real first due -------------------------

    #[test]
    fn defect6_first_due_is_today_when_the_time_is_still_ahead() {
        let now = local(2026, 8, 28, 8, 0);
        let at_nine = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let first = first_recurring_due(now, at_nine, "daily");
        assert_eq!(
            first.date_naive(),
            NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
            "a daily 09:00 reminder created at 08:00 must fire THIS morning, not tomorrow"
        );
        assert_eq!(first.format("%H:%M").to_string(), "09:00");
    }

    #[test]
    fn defect6_first_due_advances_by_the_schedule_when_the_time_has_passed() {
        let now = local(2026, 8, 28, 10, 0);
        let at_nine = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert_eq!(
            first_recurring_due(now, at_nine, "daily").date_naive(),
            NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()
        );
        // The old code hardcoded "tomorrow" for every schedule; weekly and monthly must
        // advance by their own cadence instead.
        assert_eq!(
            first_recurring_due(now, at_nine, "weekly").date_naive(),
            NaiveDate::from_ymd_opt(2026, 9, 4).unwrap()
        );
        assert_eq!(
            first_recurring_due(now, at_nine, "monthly").date_naive(),
            NaiveDate::from_ymd_opt(2026, 9, 27).unwrap()
        );
    }

    #[test]
    fn defect6_first_due_is_always_in_the_future() {
        let now = local(2026, 8, 28, 9, 0);
        for schedule in ["daily", "weekly", "monthly", "hourly", "unknown"] {
            let first =
                first_recurring_due(now, NaiveTime::from_hms_opt(9, 0, 0).unwrap(), schedule);
            assert!(
                first > now,
                "{schedule} first due {first} is not ahead of {now}"
            );
        }
    }

    // -- DEFECT 7: a never-completed recurrence never re-armed ---------------------------

    fn recurring(id: &str, due: DateTime<Utc>, schedule: &str) -> Reminder {
        Reminder {
            id: id.to_string(),
            text: format!("reminder-{id}"),
            due: due.to_rfc3339(),
            created: due.to_rfc3339(),
            completed: None,
            recurring: Some(schedule.to_string()),
            status: "pending".to_string(),
            missed_occurrences: 0,
            last_rearmed: None,
        }
    }

    #[test]
    fn defect7_stale_recurrence_catches_up_but_stays_due() {
        // Daily 09:00 reminder, never completed, five days later at 14:00.
        let mut store = ReminderStore {
            reminders: vec![recurring("a", utc(2026, 8, 24, 9, 0), "daily")],
        };
        let now = utc(2026, 8, 29, 14, 0);
        assert_eq!(rearm_recurring(&mut store, now), 1);

        let r = &store.reminders[0];
        assert_eq!(
            r.due,
            utc(2026, 8, 29, 9, 0).to_rfc3339(),
            "must catch up to the MOST RECENT occurrence, not the original and not the future"
        );
        assert!(
            DateTime::parse_from_rfc3339(&r.due).unwrap() <= now,
            "the reminder must still read as due, or re-arming would hide it"
        );
        assert_eq!(r.missed_occurrences, 5, "skipped occurrences are preserved");
        assert!(r.last_rearmed.is_some());
    }

    #[test]
    fn defect7_rearm_never_pushes_a_reminder_into_the_future() {
        let now = utc(2026, 8, 29, 8, 0);
        // Yesterday's 09:00 fired; today's has not yet.
        let mut store = ReminderStore {
            reminders: vec![recurring("a", utc(2026, 8, 24, 9, 0), "daily")],
        };
        rearm_recurring(&mut store, now);
        assert_eq!(store.reminders[0].due, utc(2026, 8, 28, 9, 0).to_rfc3339());
        assert!(DateTime::parse_from_rfc3339(&store.reminders[0].due).unwrap() <= now);
    }

    #[test]
    fn defect7_rearm_is_idempotent_and_leaves_current_reminders_alone() {
        let now = utc(2026, 8, 29, 14, 0);
        let mut store = ReminderStore {
            reminders: vec![recurring("a", utc(2026, 8, 24, 9, 0), "daily")],
        };
        rearm_recurring(&mut store, now);
        let after_first = store.reminders[0].due.clone();

        // A second pass at the same instant must change nothing and must not inflate
        // the missed count, or every read would look like a fresh miss.
        assert_eq!(rearm_recurring(&mut store, now), 0);
        assert_eq!(store.reminders[0].due, after_first);
        assert_eq!(store.reminders[0].missed_occurrences, 5);
    }

    #[test]
    fn defect7_rearm_skips_non_recurring_completed_and_future_reminders() {
        let now = utc(2026, 8, 29, 14, 0);
        let mut one_shot = recurring("one-shot", utc(2026, 8, 24, 9, 0), "daily");
        one_shot.recurring = None;
        let mut done = recurring("done", utc(2026, 8, 24, 9, 0), "daily");
        done.status = "completed".to_string();
        let future = recurring("future", utc(2026, 9, 5, 9, 0), "daily");
        let mut malformed = recurring("malformed", utc(2026, 8, 24, 9, 0), "daily");
        malformed.due = "not-a-timestamp".to_string();

        let mut store = ReminderStore {
            reminders: vec![one_shot, done, future, malformed],
        };
        assert_eq!(rearm_recurring(&mut store, now), 0);
        // A one-shot reminder that is simply overdue must stay overdue and visible.
        assert_eq!(store.reminders[0].due, utc(2026, 8, 24, 9, 0).to_rfc3339());
        assert_eq!(store.reminders[3].due, "not-a-timestamp");
    }

    #[test]
    fn defect7_completing_a_stale_recurrence_yields_a_future_occurrence() {
        // The companion bug: stepping one interval off a stale due instant minted a
        // replacement that was itself already overdue, re-stranding the reminder.
        let stale = utc(2026, 5, 1, 9, 0);
        let now = utc(2026, 8, 29, 14, 0);
        let (next, skipped) = advance_past(stale, "daily", now);
        assert!(next > now, "the next occurrence must be in the future");
        assert_eq!(next, utc(2026, 8, 30, 9, 0));
        assert_eq!(skipped, 121);
    }

    #[test]
    fn defect7_advance_past_steps_once_for_an_ordinary_completion() {
        let due = utc(2026, 8, 29, 9, 0);
        let now = utc(2026, 8, 29, 9, 5);
        let (next, skipped) = advance_past(due, "daily", now);
        assert_eq!(next, utc(2026, 8, 30, 9, 0));
        assert_eq!(skipped, 1, "an on-time completion skips nothing");

        // Completing early leaves the already-future occurrence untouched.
        let (next, skipped) = advance_past(due, "daily", utc(2026, 8, 29, 8, 0));
        assert_eq!(next, due);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn defect7_catch_up_and_advance_respect_each_schedule() {
        let from = utc(2026, 8, 1, 9, 0);
        let now = utc(2026, 8, 29, 14, 0);
        assert_eq!(catch_up_to(from, "weekly", now).0, utc(2026, 8, 29, 9, 0));
        assert_eq!(catch_up_to(from, "weekly", now).1, 4);
        assert_eq!(catch_up_to(from, "monthly", now).0, from);
        assert_eq!(advance_past(from, "monthly", now).0, utc(2026, 8, 31, 9, 0));
        assert_eq!(advance_past(from, "hourly", now).0, utc(2026, 8, 29, 15, 0));
    }

    #[test]
    fn defect7_missed_occurrences_default_for_stores_written_before_this_field_existed() {
        // Existing reminder stores must load unchanged; the new fields are additive.
        let legacy = r#"{"reminders":[{"id":"abc12345","text":"legacy","due":"2026-08-24T09:00:00+00:00","created":"2026-08-24T09:00:00+00:00","completed":null,"recurring":"daily","status":"pending"}]}"#;
        let store: ReminderStore = serde_json::from_str(legacy).expect("legacy store must load");
        assert_eq!(store.reminders[0].missed_occurrences, 0);
        assert!(store.reminders[0].last_rearmed.is_none());

        // And a reminder that has never been re-armed must not gain a null field.
        let round_trip = serde_json::to_string(&store).unwrap();
        assert!(
            !round_trip.contains("last_rearmed"),
            "unused field should be omitted: {round_trip}"
        );
    }
}
