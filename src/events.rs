use std::str::FromStr;

use crate::types::Error;
use anyhow::anyhow;
use cached::proc_macro::cached;
use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventLike,
};
use reqwest::StatusCode;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::time::Duration;
use warp::{Filter, Reply, filters::BoxedFilter, reject};

async fn fetch_calendar(calendar_url: &str) -> anyhow::Result<Calendar> {
    let calendar_request = reqwest::get(calendar_url).await?;
    let calendar_text = calendar_request.text().await?;
    let calendar = Calendar::from_str(&calendar_text).map_err(|a| anyhow!(a))?;
    Ok(calendar)
}

#[derive(Serialize, Clone)]
struct Location {
    string: String,
    url: String,
}

#[skip_serializing_none]
#[derive(Serialize, Clone)]
struct Event {
    summary: String,
    date: String,
    start_iso8601: String,
    end_iso8601: String,
    location: Option<Location>,
    description: Option<String>,
}

#[derive(Debug)]
enum EventDate {
    Date(NaiveDate),
    DateTimeUtc(DateTime<Utc>),
}

fn to_event_date(datetime: DatePerhapsTime) -> Option<EventDate> {
    match datetime {
        DatePerhapsTime::Date(naive_date) => Some(EventDate::Date(naive_date)),
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(date_time)) => {
            Some(EventDate::DateTimeUtc(date_time))
        }
        DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
            date_time: naive_date_time,
            tzid,
        }) => {
            let tz = match tzid.parse::<Tz>() {
                Ok(tz) => tz,
                // Skip if timezone is not found
                _ => return None,
            };
            // TODO: Remove unwraps
            let date_time: DateTime<Tz> = tz.from_local_datetime(&naive_date_time).unwrap();
            let date_time_utc: DateTime<Utc> =
                DateTime::<Utc>::from_timestamp(date_time.timestamp(), 0).unwrap();
            Some(EventDate::DateTimeUtc(date_time_utc))
        }
        date_perhaps_time => {
            eprintln!("Unhandled timestamp type: {:?}", date_perhaps_time);
            None
        }
    }
}

#[derive(Clone)]
struct Space {
    space_label: String,
    id: String,
}

async fn fetch_spaces() -> anyhow::Result<Vec<Space>> {
    let url: &'static str = "https://navi.jyu.fi/api/spaces";
    let request = reqwest::get(url).await?;
    let text_content = request.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text_content)?;

    let spaces = json["items"]
        .as_array()
        .ok_or_else(|| anyhow!("spaces are expressed in an unrecognized format"))?;
    let parsed_spaces = spaces
        .iter()
        .flat_map(|value| {
            value
                .as_object()
                .map(|dict| match (dict.get("spaceLabel"), dict.get("id")) {
                    (
                        Some(serde_json::Value::String(space_label)),
                        Some(serde_json::Value::String(id)),
                    ) if !space_label.is_empty() => vec![Space {
                        space_label: space_label.to_string(),
                        id: id.to_string(),
                    }],
                    _ => vec![],
                })
                .unwrap_or_else(std::vec::Vec::new)
        })
        .collect::<Vec<Space>>();
    Ok(parsed_spaces)
}

fn url_for_location(location: &str, spaces: &Vec<Space>) -> String {
    // navi.jyu.fi links for locations begining with university space codes (case sensitive!)
    for space in spaces {
        if location.starts_with(&space.space_label) {
            return format!("https://navi.jyu.fi/space/{}", space.id);
        }
    }

    // Link to Open Street Map by default
    format!(
        "https://www.google.com/maps/search/?api=1&query={}",
        urlencoding::encode(location)
    )
}

#[cached(
    time = 600,
    time_refresh = true,
    sync_writes = "default",
    result = true
)]
async fn get_events() -> Result<Vec<Event>, warp::Rejection> {
    let calendar_result = fetch_calendar(
        "https://calendar.google.com/calendar/ical/c_g2eqt2a7u1fc1pahe2o0ecm7as%40group.calendar.google.com/public/basic.ics"
    ).await;
    let calendar = match calendar_result {
        Ok(calendar) => calendar,
        Err(err) => {
            return Err(reject::custom(Error {
                message: "The remote calendar could not be processed.".to_string(),
                details: Some(format! {"{:?}", err}),
            }));
        }
    };

    let spaces = fetch_spaces().await.unwrap_or_default();

    let mut event_components: Vec<&icalendar::Event> = calendar
        .iter()
        // Filter out components other than event
        .flat_map(|component| match component {
            CalendarComponent::Event(event) => vec![event],
            _ => vec![],
        })
        // Filter old events out
        .filter(|event| {
            let current_time: DateTime<Local> = Local::now();
            match event.get_end().map(to_event_date) {
                Some(Some(end_time)) => match end_time {
                    EventDate::Date(end_date) => {
                        current_time.num_days_from_ce() <= end_date.num_days_from_ce()
                    }
                    EventDate::DateTimeUtc(end_time) => {
                        current_time.timestamp() <= end_time.timestamp()
                    }
                },
                _ => false,
            }
        })
        .collect();

    event_components.sort_by_key(|event| {
        match event.get_end().map(to_event_date) {
            Some(Some(end_time)) => {
                match end_time {
                    EventDate::Date(end_date) => {
                        let end_date_local = Local
                            .with_ymd_and_hms(
                                end_date.year(),
                                end_date.month(),
                                end_date.day(),
                                0,
                                0,
                                0,
                            )
                            .unwrap(); // TODO: Remove unwrap
                        end_date_local.timestamp()
                    }
                    EventDate::DateTimeUtc(end_time) => end_time.timestamp(),
                }
            }
            _ => unreachable!(),
        }
    });

    let events: Vec<Event> = event_components
        .iter()
        .flat_map(|event| {
            // Extract required values from event
            let (summary, start, end) = match (
                event.get_summary().map(String::from),
                event.get_start().and_then(to_event_date),
                event.get_end().and_then(to_event_date),
            ) {
                (Some(summary), Some(start), Some(end)) => (summary, start, end),
                // Skip event if required values are missing
                _ => return vec![],
            };

            // Extract optional values from events
            let (description, location) = (
                event.get_description().map(String::from),
                event.get_location().map(String::from),
            );

            let start_iso8601;
            let end_iso8601;
            let date_string = match (&start, end) {
                (EventDate::Date(start), EventDate::Date(end)) => {
                    start_iso8601 = format!("{}", start.format("%Y-%m-%d"));
                    end_iso8601 = format!("{}", end.format("%Y-%m-%d"));
                    if end.signed_duration_since(*start).num_days() == 1 {
                        format!("{}", start.format("%d/%m/%Y"))
                    } else {
                        format!("{} - {}", start.format("%d/%m/%Y"), end.format("%d/%m/%Y"))
                    }
                }
                (EventDate::DateTimeUtc(start), EventDate::DateTimeUtc(end)) => {
                    start_iso8601 = start.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
                    end_iso8601 = end.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
                    let local_start = start.with_timezone(&Local);
                    let local_end = end.with_timezone(&Local);
                    if local_end.signed_duration_since(local_start).num_days() < 1 {
                        format!(
                            "{} {} - {}",
                            local_start.format("%d/%m/%Y"),
                            local_start.format("%H:%M"),
                            local_end.format("%H:%M")
                        )
                    } else {
                        format!(
                            "{} - {}",
                            local_start.format("%d/%m/%Y %H:%M"),
                            local_end.format("%d/%m %H:%M")
                        )
                    }
                }
                // Skip if event start and end are expressed in different format, or when parsing fails
                _ => return vec![],
            };

            let location_with_link = location.map(|location| Location {
                url: url_for_location(&location, &spaces),
                string: location,
            });

            vec![Event {
                summary,
                description,
                date: date_string,
                start_iso8601,
                end_iso8601,
                location: location_with_link,
            }]
        })
        .collect();

    Ok(events)
}

async fn events() -> Result<impl Reply, warp::Rejection> {
    let events = get_events().await?;
    let json = warp::reply::json(&events);
    Ok(warp::reply::with_status(json, StatusCode::OK))
}

pub fn filter() -> BoxedFilter<(impl Reply,)> {
    warp::path("events").and_then(events).boxed()
}
