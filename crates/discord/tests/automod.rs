//! Auto moderation: rule bodies, the rule calls, the one-per-guild limit, the events.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::model::{AutoModActionType, AutoModKeywordPreset, AutoModTriggerType};
use discord::prelude::*;
use dr_mock::{Method, MockServer};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn rules_are_created_edited_and_deleted() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let alerts = world.add_channel(guild, "mod-log");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::AUTO_MODERATION_CONFIGURATION | Intents::AUTO_MODERATION_EXECUTION)
        .on(AutoModerationRuleCreate, |ctx, r| async move {
            ctx.state.lock().push(format!("create {}", r.name));
            Ok(())
        })
        .on(AutoModerationRuleUpdate, |ctx, r| async move {
            ctx.state.lock().push(format!("update {} enabled={}", r.name, r.enabled));
            Ok(())
        })
        .on(AutoModerationRuleDelete, |ctx, r| async move {
            ctx.state.lock().push(format!("delete {}", r.name));
            Ok(())
        })
        .on(AutoModerationActionExecution, |ctx, e| async move {
            ctx.state.lock().push(format!("action {} on {:?}", u16::from(e.action.kind), e.matched_keyword));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;
    let automod = client.guild(guild).automod();

    let body = CreateAutoModRule::keyword("No spoilers", ["*spoiler*", "ending"])
        .regex([r"s\d+e\d+"])
        .allow(["spoilerfree"])
        .action(AutoModAction::block(Some("No spoilers here".into())))
        .action(AutoModAction::alert(alerts))
        .action(AutoModAction::timeout(60))
        .exempt_roles([RoleId::new(9)]);
    let rule = automod.create(body).reason("spoilers").await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!(
        (request.method.clone(), request.path.as_str()),
        (Method::POST, &*format!("/guilds/{guild}/auto-moderation/rules"))
    );
    assert_eq!(
        request.json(),
        json!({
            "name": "No spoilers",
            "event_type": 1,
            "trigger_type": 1,
            "trigger_metadata": {"keyword_filter": ["*spoiler*", "ending"], "regex_patterns": ["s\\d+e\\d+"], "allow_list": ["spoilerfree"]},
            "actions": [
                {"type": 1, "metadata": {"custom_message": "No spoilers here"}},
                {"type": 2, "metadata": {"channel_id": alerts.to_string()}},
                {"type": 3, "metadata": {"duration_seconds": 60}},
            ],
            "enabled": true,
            "exempt_roles": ["9"],
        })
    );
    assert_eq!(rule.trigger_type, AutoModTriggerType::KEYWORD);
    assert_eq!(rule.actions[1].kind, AutoModActionType::SEND_ALERT_MESSAGE);
    assert_eq!(rule.trigger_metadata.keyword_filter.len(), 2);

    let presets =
        CreateAutoModRule::keyword_preset("Lists", [AutoModKeywordPreset::SLURS]).action(AutoModAction::block(None));
    automod.create(presets.clone()).await.unwrap();
    let error = automod.create(presets).await.unwrap_err();
    assert!(error.code().is_some(), "a guild has one preset rule: {error:?}");

    let off = automod.edit(rule.id, EditAutoModRule::new().enabled(false)).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json(), json!({"enabled": false}));
    assert!(!off.enabled);
    assert_eq!(automod.fetch_all().await.unwrap().len(), 2);
    assert_eq!(automod.fetch(rule.id).await.unwrap().name, "No spoilers");

    world.emit(
        "AUTO_MODERATION_ACTION_EXECUTION",
        json!({
            "guild_id": guild.to_string(), "action": {"type": 1}, "rule_id": rule.id.to_string(), "rule_trigger_type": 1,
            "user_id": "5", "channel_id": alerts.to_string(), "content": "the ending", "matched_keyword": "ending",
            "matched_content": "ending",
        }),
    );
    automod.delete(rule.id).await.unwrap();
    wait_until("every event", || seen.lock().len() == 5).await;
    assert_eq!(
        *seen.lock(),
        [
            "create No spoilers",
            "create Lists",
            "update No spoilers enabled=false",
            "action 1 on Some(\"ending\")",
            "delete No spoilers"
        ]
    );
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn bad_rules_send_nothing() {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    let automod = client.guild(GuildId::new(1)).automod();
    let path = |e: Error| match e {
        Error::Validation(v) => v.path,
        other => panic!("{other:?}"),
    };
    let block = || AutoModAction::block(None);
    let cases = [
        (CreateAutoModRule::keyword("k", ["x"]), "actions"),
        (CreateAutoModRule::keyword("", ["x"]).action(block()), "name"),
        (CreateAutoModRule::keyword("k", Vec::<String>::new()).action(block()), "trigger_metadata"),
        (CreateAutoModRule::keyword("k", ["x".repeat(61)]).action(block()), "trigger_metadata.keyword_filter[0]"),
        (
            CreateAutoModRule::keyword("k", ["x"]).regex(vec!["a"; 11]).action(block()),
            "trigger_metadata.regex_patterns",
        ),
        (CreateAutoModRule::keyword_preset("p", []).action(block()), "trigger_metadata.presets"),
        (CreateAutoModRule::mention_spam("m", 51).action(block()), "trigger_metadata.mention_total_limit"),
        (
            CreateAutoModRule::spam("s").action(AutoModAction::timeout(2_419_201)),
            "actions[0].metadata.duration_seconds",
        ),
        (
            CreateAutoModRule::spam("s").action(AutoModAction::block(Some("x".repeat(151)))),
            "actions[0].metadata.custom_message",
        ),
        (CreateAutoModRule::spam("s").action(block()).exempt_roles((1..=21).map(RoleId::new)), "exempt_roles"),
    ];
    for (body, expected) in cases {
        assert_eq!(path(automod.create(body).await.unwrap_err()), expected);
    }
    let e = automod.edit(discord::model::AutoModRuleId::new(1), EditAutoModRule::new().actions([])).await.unwrap_err();
    assert_eq!(path(e), "actions");
    assert!(server.requests().is_empty());
}
