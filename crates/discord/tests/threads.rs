//! Threads and forum posts: creation, members, archives, and their events in the cache.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use discord::model::DispatchKind as K;
use discord::model::{GenericId, ThreadAutoArchiveDuration};
use discord::prelude::*;
use dr_mock::{Method, MockServer, RecordedRequest};
use serde_json::json;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

fn last(server: &MockServer) -> RecordedRequest {
    server.requests().pop().unwrap()
}

#[tokio::test]
async fn a_thread_starts_from_a_message() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let message = client.channel(channel).send("let us talk").await.unwrap();

    let body = CreateThread::new("Talk").auto_archive_duration(ThreadAutoArchiveDuration::ONE_DAY);
    let thread = message.start_thread(body).await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/channels/{channel}/messages/{}/threads", message.id));
    assert_eq!(request.json(), json!({"name": "Talk", "auto_archive_duration": 1440}));
    assert_eq!(thread.id.get(), message.id.get(), "the thread takes the message's id");
    assert_eq!(thread.kind, ChannelType::PUBLIC_THREAD);
    assert_eq!(thread.parent_id, Some(channel));
    assert!(thread.is_thread());
}

#[tokio::test]
async fn a_private_thread_and_its_members() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let alice = server.world().add_user("alice");

    let thread = client.channel(channel).threads().create(CreateThread::new("Secret").private().invitable(false));
    let thread = thread.await.unwrap();
    assert_eq!(last(&server).json(), json!({"name": "Secret", "type": 12, "invitable": false}));
    assert_eq!(thread.kind, ChannelType::PRIVATE_THREAD);

    let members = thread.handle().thread_members();
    members.add(alice).await.unwrap();
    let request = last(&server);
    assert_eq!(
        (request.method.clone(), request.path.as_str()),
        (Method::PUT, &*format!("/channels/{}/thread-members/{alice}", thread.id))
    );
    assert_eq!(members.fetch(alice).await.unwrap().user_id, Some(alice));
    assert_eq!(members.list(ThreadMemberPage::new().limit(10)).await.unwrap().len(), 2, "the bot and alice");
    assert_eq!(last(&server).query.as_deref(), Some("limit=10"));

    members.remove(alice).await.unwrap();
    assert!(members.fetch(alice).await.is_err());
    members.leave().await.unwrap();
    assert_eq!(last(&server).path, format!("/channels/{}/thread-members/%40me", thread.id));
    members.join().await.unwrap();
    assert_eq!(members.list(ThreadMemberPage::new()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn archived_and_active_threads_are_listed_apart() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let threads = client.channel(channel).threads();
    let open = threads.create(CreateThread::new("open")).await.unwrap();
    let old = threads.create(CreateThread::new("old")).await.unwrap();
    let secret = threads.create(CreateThread::new("secret").private()).await.unwrap();

    let archived = old.handle().edit(EditChannel::new().archived(true).locked(true)).await.unwrap();
    assert_eq!(last(&server).json(), json!({"archived": true, "locked": true}));
    let metadata = archived.thread_metadata.as_ref().unwrap();
    assert!(metadata.archived && metadata.locked);
    secret.handle().edit(EditChannel::new().archived(true)).await.unwrap();

    let public = threads.archived(ArchivedThreadPage::new().limit(5)).await.unwrap();
    let names: Vec<_> = public.threads.iter().map(|t| t.name.as_deref().unwrap()).collect();
    assert_eq!(names, ["old"]);
    assert_eq!(last(&server).path, format!("/channels/{channel}/threads/archived/public"));
    assert_eq!(threads.archived_private(ArchivedThreadPage::new()).await.unwrap().threads[0].id, secret.id);
    assert_eq!(threads.joined_archived(ArchivedThreadPage::new()).await.unwrap().threads.len(), 1);
    assert_eq!(last(&server).path, format!("/channels/{channel}/users/%40me/threads/archived/private"));

    let active = client.guild(guild).active_threads().await.unwrap();
    assert_eq!(active.threads.iter().map(|t| t.id).collect::<Vec<_>>(), [open.id]);
    assert_eq!(active.members.len(), 1, "the bot's membership");
}

#[tokio::test]
async fn a_forum_post_carries_its_message_and_files() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let forum = client.guild(guild).create_channel(CreateChannel::new("help").kind(ChannelType::GUILD_FORUM));
    let forum = forum.await.unwrap();

    let first = CreateMessage::new().content("It crashes").attach(CreateAttachment::bytes("log", "log.txt"));
    let post = CreateForumPost::new("Crash on start", first).tag(GenericId::new(7));
    let thread = forum.handle().threads().create_post(post).await.unwrap();

    let request = last(&server);
    assert!(request.header("content-type").unwrap().starts_with("multipart/form-data"));
    let payload = request.json();
    assert_eq!(payload["name"], "Crash on start");
    assert_eq!(payload["applied_tags"], json!(["7"]));
    assert_eq!(payload["message"]["content"], "It crashes");
    assert_eq!(payload["message"]["attachments"][0]["filename"], "log.txt");
    assert_eq!(request.files()[0].data, b"log");
    assert_eq!(thread.parent_id, Some(forum.id));

    let empty = CreateForumPost::new("x", CreateMessage::new());
    let sent = server.requests().len();
    assert!(forum.handle().threads().create_post(empty).await.is_err(), "a post needs a first message");
    assert_eq!(server.requests().len(), sent);
}

#[tokio::test]
async fn thread_events_keep_the_cache_in_step() {
    let (_server, client) = fixture().await;
    let deliver = |kind, t: &'static str, body: serde_json::Value| {
        let client = client.clone();
        async move { discord::events::deliver(&client, 0, kind, t, &body.to_string()).await }
    };
    let guild = json!({"id": "1", "name": "g", "channels": [{"id": "2", "type": 0, "name": "general"}],
                       "threads": [{"id": "3", "type": 11, "parent_id": "2", "name": "old"}]});
    deliver(K::GuildCreate, "GUILD_CREATE", guild).await;
    let general = client.channel(ChannelId::new(2));
    assert_eq!(general.threads().cached().len(), 1, "threads come with the guild");

    let thread = json!({"id": "4", "type": 11, "guild_id": "1", "parent_id": "2", "name": "new"});
    deliver(K::ThreadCreate, "THREAD_CREATE", thread).await;
    assert_eq!(general.threads().cached().len(), 2);

    let renamed = json!({"id": "4", "type": 11, "guild_id": "1", "parent_id": "2", "name": "renamed"});
    deliver(K::ThreadUpdate, "THREAD_UPDATE", renamed).await;
    assert_eq!(client.channel(ChannelId::new(4)).cached().unwrap().name.as_deref(), Some("renamed"));

    deliver(K::ThreadDelete, "THREAD_DELETE", json!({"id": "4", "type": 11, "guild_id": "1", "parent_id": "2"})).await;
    assert!(client.channel(ChannelId::new(4)).cached().is_none());

    let sync = json!({"guild_id": "1", "channel_ids": ["2"], "threads": [{"id": "5", "type": 11, "parent_id": "2"}]});
    deliver(K::ThreadListSync, "THREAD_LIST_SYNC", sync).await;
    let ids: Vec<_> = general.threads().cached().iter().map(|t| t.id.get()).collect();
    assert_eq!(ids, [5], "the sync replaced the threads of the channel");
    assert!(client.channel(ChannelId::new(2)).cached().is_some(), "and kept the channel");
}
