//! Audit log reads: filters, the entries and their changes, the entry event.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::model::{AuditLogEntryId, AuditLogEvent};
use discord::prelude::*;
use dr_mock::MockServer;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn the_audit_log_is_read_with_filters_and_entries_arrive_as_events() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let alice = world.add_user("alice");
    let bob = world.add_user("bob");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_MODERATION)
        .on(GuildAuditLogEntryCreate, |ctx, e| async move {
            ctx.state.lock().push((e.guild_id, u16::from(e.entry.action_type), e.entry.user_id));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;

    let rename = json!([{"key": "name", "old_value": "general", "new_value": "lobby"}]);
    let first = world.add_audit_entry(guild.get(), 11, alice.get(), Some(42), rename);
    let roles = json!([{"key": "$add", "new_value": [{"id": "7", "name": "Mod"}]}]);
    world.add_audit_entry(guild.get(), 25, bob.get(), Some(alice.get()), roles);
    world.add_audit_entry(guild.get(), 22, alice.get(), Some(bob.get()), json!([]));

    let log = client.guild(guild).audit_log(AuditLogQuery::new()).await.unwrap();
    let kinds: Vec<_> = log.audit_log_entries.iter().map(|e| e.action_type).collect();
    assert_eq!(
        kinds,
        [AuditLogEvent::MEMBER_BAN_ADD, AuditLogEvent::MEMBER_ROLE_UPDATE, AuditLogEvent::CHANNEL_UPDATE]
    );
    assert_eq!(log.user(alice).map(|u| u.username.as_str()), Some("alice"));
    let renamed = &log.audit_log_entries[2];
    assert_eq!(renamed.target().map(|t| t.get()), Some(42));
    let change = renamed.change("name").unwrap();
    assert_eq!((change.old_as::<String>(), change.new_as::<String>()), (Some("general".into()), Some("lobby".into())));
    assert_eq!(change.new_as::<u64>(), None, "a value of another type reads as None");

    let query = AuditLogQuery::new().user(alice).action(AuditLogEvent::CHANNEL_UPDATE).limit(5);
    let filtered = client.guild(guild).audit_log(query).await.unwrap();
    assert_eq!(filtered.audit_log_entries.len(), 1);
    let request = server.requests().pop().unwrap();
    assert_eq!(request.path, format!("/guilds/{guild}/audit-logs"));
    assert_eq!(request.query.as_deref(), Some(&*format!("action_type=11&limit=5&user_id={alice}")));
    let older =
        client.guild(guild).audit_log(AuditLogQuery::new().before(AuditLogEntryId::new(first + 1))).await.unwrap();
    assert_eq!(older.audit_log_entries.len(), 1);

    wait_until("three entry events", || seen.lock().len() == 3).await;
    assert_eq!(seen.lock()[0], (guild, 11, Some(alice)));
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}
