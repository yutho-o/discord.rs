//! Guild scheduled events: the three kinds, edits and status changes, interested users, the events and
//! the cache.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::Timestamp;
use discord::model::{RecurrenceFrequency, RecurrenceRule, ScheduledEventEntityType, ScheduledEventStatus, Weekday};
use discord::prelude::*;
use dr_mock::{Method, MockServer};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).unwrap()
}

#[derive(Default)]
struct Seen(Mutex<Vec<String>>);

#[tokio::test]
async fn an_event_is_planned_started_ended_and_deleted() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "Lounge");
    let alice = world.add_user("alice");
    world.add_member(guild, alice);
    let seen = Arc::new(Seen::default());
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_SCHEDULED_EVENTS)
        .on(GuildScheduledEventCreate, |ctx, e| async move {
            ctx.state.0.lock().push(format!("create {}", e.name));
            Ok(())
        })
        .on(GuildScheduledEventUpdate, |ctx, e| async move {
            let old = e.old.as_ref().map(|o| o.status);
            ctx.state.0.lock().push(format!("update {:?} -> {}", old.map(u16::from), u16::from(e.new.status)));
            Ok(())
        })
        .on(GuildScheduledEventDelete, |ctx, e| async move {
            ctx.state.0.lock().push(format!("delete, cached {}", e.cached.is_some()));
            Ok(())
        })
        .on(GuildScheduledEventUserAdd, |ctx, e| async move {
            ctx.state.0.lock().push(format!("interested {}", e.user_id));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;
    let events = client.guild(guild).scheduled_events();

    let start = at("2030-05-01T18:00:00+00:00");
    let weekly = RecurrenceRule {
        by_weekday: Some(vec![Weekday::THURSDAY]),
        ..RecurrenceRule::new(start, RecurrenceFrequency::WEEKLY, 1)
    };
    let body = CreateScheduledEvent::voice("Game night", channel, start).description("Bring snacks").recurrence(weekly);
    let event = events.create(body).reason("weekly").await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!(
        (request.method.clone(), request.path.as_str()),
        (Method::POST, &*format!("/guilds/{guild}/scheduled-events"))
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("weekly"));
    let sent = request.json();
    assert_eq!(sent["entity_type"], 2);
    assert_eq!(sent["channel_id"], channel.to_string());
    assert_eq!(sent["privacy_level"], 2);
    assert_eq!(sent["scheduled_start_time"], "2030-05-01T18:00:00.000000+00:00");
    assert_eq!(sent["recurrence_rule"]["by_weekday"], json!([3]));
    assert_eq!(sent["recurrence_rule"]["frequency"], 2);
    assert!(sent.get("scheduled_end_time").is_none());
    assert_eq!(event.entity_type, ScheduledEventEntityType::VOICE);
    assert_eq!(event.recurrence_rule.as_ref().unwrap().by_weekday, Some(vec![Weekday::THURSDAY]));
    wait_until("cached", || events.get(event.id).is_some()).await;

    world.set_event_interest(guild.get(), event.id.get(), alice.get(), true);
    let fetched = events.fetch(event.id).await.unwrap();
    assert_eq!(fetched.user_count, Some(1));
    assert_eq!(server.requests().pop().unwrap().query.as_deref(), Some("with_user_count=true"));
    let users = events.users(event.id, EventUserPage::new().with_member()).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].user.username, "alice");
    assert!(users[0].member.is_some());

    let started = events.start(event.id).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json(), json!({"status": 2}));
    assert_eq!(started.status, ScheduledEventStatus::ACTIVE);
    let error = events.cancel(event.id).await.unwrap_err();
    assert!(error.code().is_some(), "an active event cannot be canceled: {error:?}");
    events.end(event.id).await.unwrap();
    let moved = events
        .edit(event.id, EditScheduledEvent::new().name("Board games").clear_description().clear_recurrence())
        .await
        .unwrap();
    assert_eq!(
        server.requests().pop().unwrap().json(),
        json!({"name": "Board games", "description": null, "recurrence_rule": null})
    );
    assert_eq!(moved.description, None);

    events.delete(event.id).await.unwrap();
    wait_until("gone from the cache", || events.get(event.id).is_none()).await;
    wait_until("every event", || seen.0.lock().len() == 6).await;
    assert_eq!(
        *seen.0.lock(),
        [
            "create Game night".to_owned(),
            format!("interested {alice}"),
            "update Some(1) -> 2".to_owned(),
            "update Some(2) -> 3".to_owned(),
            "update Some(3) -> 3".to_owned(),
            "delete, cached true".to_owned(),
        ]
    );
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn external_events_and_bad_bodies() {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    let guild = server.world().add_guild("g");
    let events = client.guild(guild).scheduled_events();
    let (start, end) = (at("2030-05-01T18:00:00Z"), at("2030-05-01T20:00:00Z"));

    let event = events.create(CreateScheduledEvent::external("Meetup", "The park", start, end)).await.unwrap();
    let sent = server.requests().pop().unwrap().json();
    assert_eq!(sent["entity_type"], 3);
    assert_eq!(sent["entity_metadata"], json!({"location": "The park"}));
    assert_eq!(sent["scheduled_end_time"], "2030-05-01T20:00:00.000000+00:00");
    assert!(sent.get("channel_id").is_none());
    assert_eq!(event.entity_metadata.as_ref().unwrap().location.as_deref(), Some("The park"));
    assert_eq!(events.fetch_all().await.unwrap().len(), 1);

    let to_voice = EditScheduledEvent::new().voice(ChannelId::new(5));
    events.edit(event.id, to_voice).await.unwrap();
    assert_eq!(
        server.requests().pop().unwrap().json(),
        json!({"entity_type": 2, "channel_id": "5", "entity_metadata": null})
    );

    let before = server.requests().len();
    let path = |e: Error| match e {
        Error::Validation(v) => v.path,
        other => panic!("{other:?}"),
    };
    let e = events.create(CreateScheduledEvent::external("", "x", start, end)).await.unwrap_err();
    assert_eq!(path(e), "name");
    let e = events.create(CreateScheduledEvent::external("M", "", start, end)).await.unwrap_err();
    assert_eq!(path(e), "entity_metadata.location");
    let e = events.create(CreateScheduledEvent::external("M", "x", end, start)).await.unwrap_err();
    assert_eq!(path(e), "scheduled_end_time");
    let long = "x".repeat(1001);
    let e =
        events.create(CreateScheduledEvent::voice("M", ChannelId::new(5), start).description(long)).await.unwrap_err();
    assert_eq!(path(e), "description");
    let e = events.edit(event.id, EditScheduledEvent::new().start(end).end(start)).await.unwrap_err();
    assert_eq!(path(e), "scheduled_end_time");
    assert_eq!(server.requests().len(), before, "nothing was sent");
}
