use std::str::FromStr;

use crate::types::Error;
use anyhow::anyhow;
use chrono::{Date, DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::{Tz, UTC};
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventLike,
};
use reqwest::StatusCode;
use serde::Serialize;
use warp::{Filter, Reply, filters::BoxedFilter, reject};

async fn fetch_calendar(calendar_url: &str) -> anyhow::Result<Calendar> {
    let calendar_request = reqwest::get(calendar_url).await?;
    let calendar_text = calendar_request.text().await?;
    let calendar = Calendar::from_str(&calendar_text).map_err(|a| anyhow!(a))?;
    Ok(calendar)
}

#[derive(Serialize)]
struct Location {
    string: String,
    url: String,
}

#[derive(Serialize)]
struct Event {
    summary: String,
    date: String,
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

async fn events(amount: usize) -> Result<impl Reply, warp::Rejection> {
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
                Some(Some(end_time)) => {
                    match end_time {
                        EventDate::Date(end_date) => {
                            current_time.num_days_from_ce() <= end_date.num_days_from_ce()
                        }
                        EventDate::DateTimeUtc(end_time) => {
                            current_time.timestamp() <= end_time.timestamp()
                        }
                    }
                }
                _ => false
            }
        })
        .collect();

    event_components.sort_by_key(|event| {
        match event.get_end().map(to_event_date) {
            Some(Some(end_time)) => {
                match end_time {
                    EventDate::Date(end_date) => {
                        let end_date_local = Local.with_ymd_and_hms(end_date.year(), end_date.month(), end_date.day(), 0, 0, 0).unwrap(); // TODO: Remove unwrap
                        end_date_local.timestamp()
                    }
                    EventDate::DateTimeUtc(end_time) => {
                        end_time.timestamp()
                    }
                }
            }
            _ => unreachable!()
        }
    });

    let events: Vec<Event> = event_components
        .iter()
        .take(amount)
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
            println!("{summary}: start: {:?} end: {:?}", start, end);

            // Extract optional values from events
            let (description, location) = (
                event.get_description().map(String::from),
                event.get_location().map(String::from),
            );

            let date_string = match (start, end) {
                (EventDate::Date(start), EventDate::Date(end)) => {
                    if end.signed_duration_since(start).num_days() == 1 {
                        format!("{}", start.format("%d/%m/%Y"))
                    } else {
                        format!("{} - {}", start.format("%d/%m/%Y"), end.format("%d/%m/%Y"))
                    }
                }
                (EventDate::DateTimeUtc(start), EventDate::DateTimeUtc(end)) => {
                    let local_start: DateTime<Local> = DateTime::from(start);
                    if end.signed_duration_since(local_start).num_days() < 1 {
                        format!(
                            "{} {} - {}",
                            start.format("%d/%m/%Y"),
                            start.format("%H:%M"),
                            end.format("%H:%M")
                        )
                    } else {
                        format!(
                            "{} - {}",
                            start.format("%d/%m/%Y %H:%M"),
                            end.format("%d/%m %H:%M")
                        )
                    }
                }
                // Skip if event start and end are expressed in different format, or when parsing fails
                _ => return vec![],
            };

            let location_with_link = location.map(|location| Location {
                url: format!("https://osm.org/search?query={}", urlencoding::encode(&location)),
                string: location,
            });

            vec![Event {
                summary,
                description,
                date: date_string,
                location: location_with_link,
            }]
        })
        .collect();

    let json = warp::reply::json(&events);
    Ok(warp::reply::with_status(json, StatusCode::OK))
}

pub fn filter() -> BoxedFilter<(impl Reply,)> {
    warp::path("events")
        .and(warp::path::param().and_then(events))
        .or(warp::any().and_then(|| events(10)))
        .boxed()
}
