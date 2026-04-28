use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Weekday};

use crate::models::{Energy, NewTask, Priority};

#[derive(Debug, Default)]
pub struct ParsedCapture {
    pub title: String,
    pub due_at: Option<DateTime<Local>>,
    pub priority: Priority,
    pub energy: Energy,
    pub estimate_min: Option<i64>,
    pub tags: Vec<String>,
}

impl From<ParsedCapture> for NewTask {
    fn from(p: ParsedCapture) -> Self {
        NewTask {
            title: p.title,
            due_at: p.due_at,
            priority: p.priority,
            energy: p.energy,
            estimate_min: p.estimate_min,
            tags: p.tags,
            ..Default::default()
        }
    }
}

/// Parse a quick-capture string like:
///   "Submit report friday 4pm #work !p1 ~30m"
pub fn parse(input: &str) -> ParsedCapture {
    let mut result = ParsedCapture::default();
    let mut title_parts: Vec<&str> = Vec::new();
    let mut words = input.split_whitespace().peekable();
    let now = Local::now();

    while let Some(word) = words.next() {
        // Tags: #foo
        if let Some(tag) = word.strip_prefix('#') {
            if !tag.is_empty() {
                result.tags.push(tag.to_lowercase());
                continue;
            }
        }

        // Priority: !p0..!p4
        if let Some(p) = word.strip_prefix('!') {
            match p.to_lowercase().as_str() {
                "p0" => { result.priority = Priority::P0; continue; }
                "p1" => { result.priority = Priority::P1; continue; }
                "p2" => { result.priority = Priority::P2; continue; }
                "p3" => { result.priority = Priority::P3; continue; }
                "p4" => { result.priority = Priority::P4; continue; }
                _ => {}
            }
        }

        // Estimate: ~30m, ~1h, ~90m
        if let Some(est) = word.strip_prefix('~') {
            if let Some(mins) = parse_duration_str(est) {
                result.estimate_min = Some(mins);
                continue;
            }
        }

        // Energy: @low @medium @high
        if let Some(e) = word.strip_prefix('@') {
            match e.to_lowercase().as_str() {
                "low" | "l" => { result.energy = Energy::Low; continue; }
                "medium" | "med" | "m" => { result.energy = Energy::Medium; continue; }
                "high" | "h" => { result.energy = Energy::High; continue; }
                _ => {}
            }
        }

        // Date/time keywords
        let lower = word.to_lowercase();
        let date_result = match lower.as_str() {
            "today" => Some(now.date_naive()),
            "tomorrow" => Some((now + Duration::days(1)).date_naive()),
            "yesterday" => Some((now - Duration::days(1)).date_naive()),
            "monday" | "mon" => Some(next_weekday(now.weekday(), Weekday::Mon, &now)),
            "tuesday" | "tue" => Some(next_weekday(now.weekday(), Weekday::Tue, &now)),
            "wednesday" | "wed" => Some(next_weekday(now.weekday(), Weekday::Wed, &now)),
            "thursday" | "thu" => Some(next_weekday(now.weekday(), Weekday::Thu, &now)),
            "friday" | "fri" => Some(next_weekday(now.weekday(), Weekday::Fri, &now)),
            "saturday" | "sat" => Some(next_weekday(now.weekday(), Weekday::Sat, &now)),
            "sunday" | "sun" => Some(next_weekday(now.weekday(), Weekday::Sun, &now)),
            "eow" | "end-of-week" => {
                Some(next_weekday(now.weekday(), Weekday::Fri, &now))
            }
            "eom" | "end-of-month" => {
                use chrono::NaiveDate;
                let y = now.year();
                let m = now.month();
                let next_month = if m == 12 {
                    NaiveDate::from_ymd_opt(y + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(y, m + 1, 1)
                };
                next_month.map(|d| d - Duration::days(1))
            }
            _ => None,
        };

        if let Some(date) = date_result {
            // Look ahead for a time token like "4pm", "14:00", "9am"
            let time_hm = if let Some(&next) = words.peek() {
                if let Some(hm) = parse_time_str(next) {
                    words.next();
                    hm
                } else {
                    (9, 0)
                }
            } else {
                (9, 0)
            };
            if let Some(dt) = date.and_hms_opt(time_hm.0, time_hm.1, 0) {
                result.due_at = Local.from_local_datetime(&dt).single();
            }
            continue;
        }

        // Bare time like "4pm" "14:30" (assume today if no date set yet)
        if let Some(hm) = parse_time_str(word) {
            if result.due_at.is_none() {
                if let Some(dt) = now.date_naive().and_hms_opt(hm.0, hm.1, 0) {
                    result.due_at = Local.from_local_datetime(&dt).single();
                }
            }
            continue;
        }

        title_parts.push(word);
    }

    result.title = title_parts.join(" ");
    if result.title.is_empty() {
        result.title = input.to_string();
    }

    result
}

fn parse_time_str(s: &str) -> Option<(u32, u32)> {
    let s = s.trim().to_lowercase();
    // "4pm", "4:30pm", "16:00", "9am"
    if let Some(h_str) = s.strip_suffix("pm") {
        let (h, m) = split_hm(h_str);
        let h = h? + if h? == 12 { 0 } else { 12 };
        return Some((h.min(23), m.unwrap_or(0)));
    }
    if let Some(h_str) = s.strip_suffix("am") {
        let (h, m) = split_hm(h_str);
        let h = if h? == 12 { 0 } else { h? };
        return Some((h.min(23), m.unwrap_or(0)));
    }
    if s.contains(':') {
        let (h, m) = split_hm(&s);
        return Some((h?.min(23), m.unwrap_or(0).min(59)));
    }
    None
}

fn split_hm(s: &str) -> (Option<u32>, Option<u32>) {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    let h = parts[0].parse().ok();
    let m = parts.get(1).and_then(|p| p.parse().ok());
    (h, m)
}

fn parse_duration_str(s: &str) -> Option<i64> {
    let s = s.to_lowercase();
    if let Some(h) = s.strip_suffix('h') {
        return h.parse::<i64>().ok().map(|h| h * 60);
    }
    if let Some(m) = s.strip_suffix('m') {
        return m.parse::<i64>().ok();
    }
    // bare number treated as minutes
    s.parse::<i64>().ok()
}

fn next_weekday(current: Weekday, target: Weekday, now: &DateTime<Local>) -> chrono::NaiveDate {
    let cur_num = current.num_days_from_monday();
    let tgt_num = target.num_days_from_monday();
    let days_ahead = if tgt_num > cur_num {
        tgt_num - cur_num
    } else if tgt_num == cur_num {
        7
    } else {
        7 - cur_num + tgt_num
    };
    (now.date_naive() + Duration::days(days_ahead as i64))
}
