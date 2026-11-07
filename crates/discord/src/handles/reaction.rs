//! `Bound<MessageReactionAddEvent>` and `Bound<MessageReactionRemoveEvent>`.

use crate::handles::{ChannelHandle, GuildHandle, UserHandle};
use crate::{Bound, Call};
use dr_model::{Message, MessageReactionAddEvent, MessageReactionRemoveEvent};

macro_rules! reaction_event {
    ($event:ty) => {
        impl Bound<$event> {
            /// The channel, as a handle.
            pub fn channel(&self) -> ChannelHandle {
                self.client().channel(self.channel_id)
            }

            /// The guild, as a handle. `None` in a DM.
            pub fn guild(&self) -> Option<GuildHandle> {
                self.guild_id.map(|g| self.client().guild(g))
            }

            /// The user who reacted, as a handle.
            pub fn user(&self) -> UserHandle {
                self.client().user(self.user_id)
            }

            /// Fetches the message. The event names it and does not carry it.
            pub fn fetch_message(&self) -> Call<Bound<Message>> {
                self.channel().messages().fetch(self.message_id)
            }
        }
    };
}

reaction_event!(MessageReactionAddEvent);
reaction_event!(MessageReactionRemoveEvent);
