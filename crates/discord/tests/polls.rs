//! Polls: the body that creates one, the voters, ending it, and the vote events.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::prelude::*;
use dr_mock::{Method, MockServer};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

#[tokio::test]
async fn a_poll_is_sent_voted_on_and_ended() {
    let (server, client) = fixture().await;
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "general");
    let alice = world.add_user("alice");
    let bob = world.add_user("bob");

    let poll = PollBuilder::new("Pizza or pasta?").answer("Pizza").answer_with_emoji("Pasta", "🍝").hours(4);
    let message = client.channel(channel).send(CreateMessage::new().poll(poll.multiselect(true))).await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!(request.path, format!("/channels/{channel}/messages"));
    assert_eq!(
        request.json()["poll"],
        json!({
            "question": {"text": "Pizza or pasta?"},
            "answers": [{"poll_media": {"text": "Pizza"}}, {"poll_media": {"text": "Pasta", "emoji": {"name": "🍝"}}}],
            "duration": 4,
            "allow_multiselect": true,
        })
    );
    let sent = message.poll.as_ref().expect("the poll comes back");
    assert_eq!(sent.answers.len(), 2);
    assert_eq!(sent.answer(2).and_then(|a| a.poll_media.text.as_deref()), Some("Pasta"));
    assert!(sent.allow_multiselect);
    assert_eq!(sent.votes(1), Some(0));

    for (user, answer) in [(alice, 1), (bob, 1), (bob, 2)] {
        assert!(world.vote(channel.get(), message.id.get(), user.get(), answer, true));
    }
    let voters = message.poll_voters(1, VoterPage::new()).await.unwrap();
    let names: Vec<_> = voters.users.iter().map(|u| u.username.as_str()).collect();
    assert_eq!(names, ["alice", "bob"]);
    let request = server.requests().pop().unwrap();
    assert_eq!(request.path, format!("/channels/{channel}/polls/{}/answers/1", message.id));
    let page = message.poll_voters(1, VoterPage::new().after(alice).limit(10)).await.unwrap();
    assert_eq!(page.users.len(), 1);
    assert_eq!(server.requests().pop().unwrap().query.as_deref(), Some(&*format!("after={alice}&limit=10")));

    let ended = message.end_poll().await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!(
        (request.method, request.path),
        (Method::POST, format!("/channels/{channel}/polls/{}/expire", message.id))
    );
    let results = ended.poll.as_ref().and_then(|p| p.results.as_ref()).unwrap();
    assert!(results.is_finalized);
    assert_eq!(ended.poll.as_ref().unwrap().votes(1), Some(2));
    assert_eq!(ended.poll.as_ref().unwrap().votes(2), Some(1));
    assert!(!world.vote(channel.get(), message.id.get(), alice.get(), 2, true), "an ended poll takes no votes");
}

#[tokio::test]
async fn bad_polls_send_nothing() {
    let (server, client) = fixture().await;
    let channel = server.world().add_channel(server.world().add_guild("g"), "general");
    let long = "x".repeat(56);
    let cases = [
        (PollBuilder::new("No answers"), "poll.answers"),
        (PollBuilder::new("").answer("a"), "poll.question.text"),
        (PollBuilder::new("q").answer(&long), "poll.answers[0].poll_media.text"),
        (PollBuilder::new("q").answer("a").hours(769), "poll.duration"),
        (PollBuilder::new("q").answer("a").hours(0), "poll.duration"),
        (PollBuilder::new("q").answer_with_emoji("a", ":"), "poll.answers[0].poll_media.emoji"),
        ((1..=11).fold(PollBuilder::new("q"), |p, i| p.answer(i.to_string())), "poll.answers"),
    ];
    for (poll, path) in cases {
        let error = client.channel(channel).send(CreateMessage::new().poll(poll)).await.unwrap_err();
        let Error::Validation(v) = &error else { panic!("{error:?}") };
        assert_eq!(v.path, path);
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn votes_arrive_as_events_and_the_end_updates_the_cache() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "general");
    let alice = world.add_user("alice");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::GUILD_MESSAGE_POLLS)
        .cache(CacheConfig::standard().messages_per_channel(10))
        .on(MessagePollVoteAdd, |ctx, vote| async move {
            ctx.state.lock().push(("add", vote.user_id, vote.answer_id, vote.guild_id));
            Ok(())
        })
        .on(MessagePollVoteRemove, |ctx, vote| async move {
            ctx.state.lock().push(("remove", vote.user_id, vote.answer_id, vote.guild_id));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;

    let message =
        client.channel(channel).send(CreateMessage::new().poll(PollBuilder::new("q").answer("a"))).await.unwrap();
    wait_until("cached", || client.cache().message(channel, message.id).is_some()).await;
    world.vote(channel.get(), message.id.get(), alice.get(), 1, true);
    world.vote(channel.get(), message.id.get(), alice.get(), 1, false);
    wait_until("both votes", || seen.lock().len() == 2).await;
    assert_eq!(*seen.lock(), [("add", alice, 1, Some(guild)), ("remove", alice, 1, Some(guild))]);

    message.end_poll().await.unwrap();
    wait_until("the final results in the cache", || {
        let cached = client.cache().message(channel, message.id);
        cached.and_then(|m| m.poll.as_ref().and_then(|p| p.results.as_ref().map(|r| r.is_finalized))) == Some(true)
    })
    .await;
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}
