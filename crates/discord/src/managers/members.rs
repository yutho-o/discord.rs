//! The members of one guild.

use crate::error::ConfigError;
use crate::events::GuildMembersChunk;
use crate::{Bound, Call, Client, Error};
use dr_builders::{CreateBan, EditMember};
use dr_gateway::GatewayCommand;
use dr_model::{GuildId, Member, RoleId, UserId};
use dr_model::{Intents, RequestGuildMembers};
use dr_rest::api::UserPage;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// The members of one guild. Get one from `GuildHandle::members`.
#[derive(Clone, Debug)]
pub struct MembersManager {
    client: Client,
    guild: GuildId,
}

/// How long a request for members waits for each chunk.
const CHUNK_WAIT: Duration = Duration::from_secs(30);

/// Makes each request's nonce unique in this process.
static NONCE: AtomicU64 = AtomicU64::new(0);

impl MembersManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// Every member of the guild, over the gateway, which is the way for a large guild. Needs the
    /// `GUILD_MEMBERS` intent. The members also go to the cache.
    pub async fn fetch_all(&self) -> Result<Vec<Bound<Member>>, Error> {
        self.request(RequestGuildMembers::all(self.guild)).await
    }

    /// Members whose username or nickname starts with `query`, at most `limit` (up to 100), over
    /// the gateway.
    pub async fn search_gateway(&self, query: impl Into<String>, limit: u32) -> Result<Vec<Bound<Member>>, Error> {
        self.request(RequestGuildMembers::search(self.guild, query, limit.clamp(1, 100))).await
    }

    /// These members, over the gateway, in one request of up to 100 users. Users who are not members
    /// are left out.
    pub async fn fetch_many(&self, users: impl IntoIterator<Item = UserId>) -> Result<Vec<Bound<Member>>, Error> {
        self.request(RequestGuildMembers::users(self.guild, users)).await
    }

    /// Sends op 8 on the guild's shard and gathers every `GUILD_MEMBERS_CHUNK` of the answer. The
    /// nonce is set here. Fails with `NotReady` before the client runs and `Timeout` when a chunk
    /// does not come within 30 s.
    pub async fn request(&self, mut request: RequestGuildMembers) -> Result<Vec<Bound<Member>>, Error> {
        request.guild_id = self.guild;
        let have = self.client.0.options.intents;
        let lists_all = request.user_ids.is_none() && request.query.as_deref().is_none_or(str::is_empty);
        if lists_all && !have.contains(Intents::GUILD_MEMBERS) {
            let needs = Intents::GUILD_MEMBERS;
            return Err(ConfigError::MissingIntent { event: "GUILD_MEMBERS_CHUNK", needs, have }.into());
        }
        if request.user_ids.as_ref().is_some_and(|ids| ids.len() > 100) {
            return Err(Error::Other("a request for members names at most 100 users".into()));
        }
        let gateway = self.client.0.gateway.get().ok_or(Error::NotReady)?;
        let shard = gateway.shard_for(self.guild);
        let nonce = format!("m{}", NONCE.fetch_add(1, Ordering::Relaxed));
        request.nonce = Some(nonce.clone());
        // Collect before sending, so that no chunk can arrive first.
        let mut chunks = self.client.collector::<GuildMembersChunk>(move |c| c.nonce.as_deref() == Some(&nonce));
        gateway.send(shard, GatewayCommand::RequestGuildMembers(request))?;

        let mut members = Vec::new();
        let mut received = 0u32;
        loop {
            let chunk = chunks.next_within(CHUNK_WAIT).await.ok_or(Error::Timeout { what: "a guild members chunk" })?;
            members.extend(chunk.members.iter().cloned().map(|m| self.client.bind(m)));
            received += 1;
            if received >= chunk.chunk_count {
                return Ok(members);
            }
        }
    }

    /// The member from the cache. No I/O.
    pub fn get(&self, user: UserId) -> Option<Bound<Member>> {
        self.client.cache().member(self.guild, user).map(|m| self.client.bind_arc(m))
    }

    /// The member from Discord. The response has no `guild_id`, so this fills it in with
    /// [`Call::fix_wire`].
    pub fn fetch(&self, user: UserId) -> Call<Bound<Member>> {
        let guild = self.guild;
        Call::new(self.client.clone(), self.client.rest().guilds().get_member(guild, user))
            .fix_wire(move |member: &mut Member| member.guild_id = Some(guild))
    }

    /// The member from the cache, else from Discord.
    pub async fn resolve(&self, user: UserId) -> Result<Bound<Member>, Error> {
        if let Some(member) = self.get(user) {
            return Ok(member);
        }
        let fetched = self.fetch(user).await?;
        self.client.cache().insert_member_if_absent(self.guild, fetched.data().clone());
        Ok(fetched)
    }

    /// One page of members, ordered by user id, from Discord. Needs the `GUILD_MEMBERS` intent turned
    /// on in the developer portal.
    pub fn list(&self, page: UserPage) -> Call<Vec<Bound<Member>>> {
        let guild = self.guild;
        Call::new(self.client.clone(), self.client.rest().guilds().get_members(guild, page)).fix_wire(
            move |members: &mut Vec<Member>| {
                for member in members {
                    member.guild_id = Some(guild);
                }
            },
        )
    }

    /// Edits a member. Use `.reason(...)` for the audit log.
    pub fn edit(&self, user: UserId, body: EditMember) -> Call<Bound<Member>> {
        let guild = self.guild;
        Call::new(self.client.clone(), self.client.rest().guilds().modify_member(guild, user, body))
            .fix_wire(move |member: &mut Member| member.guild_id = Some(guild))
    }

    /// Times a member out for this long, up to 28 days. Use `.reason(...)` for the audit log.
    pub fn timeout(&self, user: UserId, duration: Duration) -> Call<Bound<Member>> {
        self.edit(user, EditMember::new().timeout_for(duration))
    }

    /// Ends a member's timeout. Use `.reason(...)` for the audit log.
    pub fn clear_timeout(&self, user: UserId) -> Call<Bound<Member>> {
        self.edit(user, EditMember::new().clear_timeout())
    }

    /// Gives a member a role. Use `.reason(...)` for the audit log.
    pub fn add_role(&self, user: UserId, role: RoleId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().add_member_role(self.guild, user, role))
    }

    /// Takes a role from a member. Use `.reason(...)` for the audit log.
    pub fn remove_role(&self, user: UserId, role: RoleId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().remove_member_role(self.guild, user, role))
    }

    /// Bans a member, deleting none of their messages. `GuildHandle::bans` has the other options. Use
    /// `.reason(...)` for the audit log.
    pub fn ban(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().create_ban(self.guild, user, CreateBan::new()))
    }

    /// Kicks a member. Use `.reason(...)` for the audit log.
    pub fn kick(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().remove_member(self.guild, user))
    }
}
