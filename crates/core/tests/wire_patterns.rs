//! Runtime proofs for every binding wire pattern of `ARCHITECTURE.md` 7.4 to 7.6. The types are
//! local to this file, so the real model can change without touching these proofs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::marker::{GuildMarker, RoleMarker, UserMarker};
use dr_core::{
    Endpoints, Id, IntoBody, Lenient, Patch, RequestBody, Secret, Timestamp, ValidationError, decode, lenient_i64,
    null_default, open_flags, open_int, open_str, tagged_union,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::value::RawValue;
use std::borrow::Cow;
use std::collections::HashMap;

type UserId = Id<UserMarker>;
type GuildId = Id<GuildMarker>;
type RoleId = Id<RoleMarker>;

open_int! {
    /// Channel kinds.
    ChannelType: u16 {
        GUILD_TEXT = 0,
        /// A documented arm.
        DM = 1,
        GUILD_ANNOUNCEMENT = 5;
        /// Upstream alias.
        alias GUILD_NEWS = GUILD_ANNOUNCEMENT;
    }
}
open_int! {
    /// Wide values need a wide repr.
    ErrorCode: u32 { MISSING_PERMISSIONS = 50013, BIG = 340024 }
}
open_str! {
    /// Guild features.
    GuildFeature {
        /// A documented arm.
        Community = "COMMUNITY",
        News = "NEWS",
    }
}
open_flags! {
    /// Number-form flags.
    MessageFlags: number {
        /// A documented bit.
        EPHEMERAL = 1 << 6,
        IS_COMPONENTS_V2 = 1 << 15,
    }
}
open_flags! {
    /// String-form flags.
    Permissions: string { ADMINISTRATOR = 1 << 3, KICK_MEMBERS = 1 << 1 }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Button {
    #[serde(default)]
    style: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    custom_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActionRow {
    #[serde(default)]
    components: Vec<Component>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct TextDisplay {
    #[serde(default)]
    content: String,
}
tagged_union! {
    #[tag = "type"]
    /// Message component.
    pub enum Component {
        1 => ActionRow(ActionRow),
        /// A documented arm.
        2 => Button(Button),
        10 => TextDisplay(TextDisplay),
        _ => Unknown,
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct User {
    id: UserId,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Message {
    id: Id<dr_core::marker::MessageMarker>,
    author: User,
    #[serde(default)]
    content: String,
    #[serde(default, deserialize_with = "null_default")]
    components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    referenced_message: Patch<Box<Message>>,
}

#[test]
fn id_forms() {
    let a: UserId = serde_json::from_str("\"80351110224678912\"").unwrap();
    let b: UserId = serde_json::from_str("80351110224678912").unwrap();
    assert_eq!(a, b);
    assert_eq!(serde_json::to_string(&a).unwrap(), "\"80351110224678912\"");
    assert_eq!(a.to_string(), "80351110224678912");
    assert_eq!("80351110224678912".parse::<UserId>().unwrap(), a);
    let z: UserId = serde_json::from_str("\"0\"").unwrap();
    assert_eq!(z.get(), 0);
    assert!(serde_json::from_str::<UserId>("-1").is_err());
    assert!(serde_json::from_str::<UserId>("\"abc\"").is_err());
    assert_eq!(serde_json::from_str::<Option<UserId>>("null").unwrap(), None);
    // as a map key, both ways: resolved data maps
    let m: HashMap<UserId, u32> = serde_json::from_str(r#"{"123":1}"#).unwrap();
    assert_eq!(m[&Id::new(123)], 1);
    assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"123":1}"#);
    // through Value too
    let v: UserId = serde_json::from_value(json!("42")).unwrap();
    assert_eq!(v.get(), 42);
    // cast and copy with an uninhabited marker, no bound on M
    let g: GuildId = Id::new(7);
    let r: RoleId = g.cast();
    let (x, y) = (r, r);
    assert_eq!(x, y);
    assert_eq!(std::mem::size_of::<UserId>(), 8);
    assert_eq!(Id::<UserMarker>::new(175_928_847_299_117_063).timestamp_ms(), 1_462_015_105_796);
}

#[test]
fn open_int_rules() {
    let t: ChannelType = serde_json::from_str("99").unwrap();
    assert_eq!(t, ChannelType(99));
    assert_eq!(serde_json::to_string(&t).unwrap(), "99");
    assert_eq!(format!("{:?} {t:?}", ChannelType::GUILD_TEXT), "ChannelType::GUILD_TEXT ChannelType(99)");
    let s = match ChannelType(5) {
        ChannelType::GUILD_TEXT => "t",
        ChannelType::GUILD_NEWS => "alias",
        _ => "o",
    };
    assert_eq!(s, "alias");
    assert_eq!(serde_json::from_str::<ChannelType>("777").unwrap().get(), 777);
    // alias prints the canonical name
    assert_eq!(ChannelType::GUILD_NEWS.name(), Some("GUILD_ANNOUNCEMENT"));
    // out of range for the repr is a decode error, so repr is chosen per enum
    assert!(serde_json::from_str::<ChannelType>("70000").is_err());
    assert_eq!(serde_json::from_str::<ErrorCode>("340024").unwrap(), ErrorCode::BIG);
    assert_eq!(std::mem::size_of::<ChannelType>(), 2);
}

#[test]
fn open_str_rules() {
    let f: GuildFeature = serde_json::from_str("\"NEW_THING\"").unwrap();
    assert!(matches!(f, GuildFeature::Unknown(_)));
    assert_eq!(f.as_str(), "NEW_THING");
    assert_eq!(serde_json::to_string(&f).unwrap(), "\"NEW_THING\"");
    let k: GuildFeature = serde_json::from_str("\"COMMUNITY\"").unwrap();
    assert_eq!(k, GuildFeature::Community);
    assert_eq!(GuildFeature::from("NEWS"), GuildFeature::News);
    // escaped form decodes to the known variant, so a borrow-only impl would have failed
    let e: GuildFeature = serde_json::from_str("\"NE\\u0057S\"").unwrap();
    assert_eq!(e, GuildFeature::News);
}

#[test]
fn flags_rules() {
    let raw = (1u64 << 40) | (1 << 6);
    let f: MessageFlags = serde_json::from_str(&raw.to_string()).unwrap();
    assert!(f.contains(MessageFlags::EPHEMERAL));
    assert_eq!(f.bits(), raw);
    assert_eq!(serde_json::to_string(&f).unwrap(), raw.to_string());
    assert_eq!(format!("{f:?}"), "MessageFlags(EPHEMERAL | 0x10000000000)");
    // also tolerate a string on a number-form field
    let s: MessageFlags = serde_json::from_str("\"64\"").unwrap();
    assert_eq!(s, MessageFlags::EPHEMERAL);
    // permissions: 2^53 and up must survive, written as a decimal string
    let big = (1u64 << 60) | (1 << 3);
    let p: Permissions = serde_json::from_str(&format!("\"{big}\"")).unwrap();
    assert!(p.contains(Permissions::ADMINISTRATOR));
    assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{big}\""));
    let q: Permissions = serde_json::from_str("8").unwrap();
    assert_eq!(q, Permissions::ADMINISTRATOR);
    // set operations do not drop unknown bits
    let u = Permissions::from_bits_retain(1 << 60) | Permissions::KICK_MEMBERS;
    assert_eq!(u.bits(), (1 << 60) | 2);
    assert_eq!((u - Permissions::KICK_MEMBERS).bits(), 1 << 60);
    assert_eq!(Permissions::empty().bits(), 0);
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct EditChannel {
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    topic: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    parent_id: Patch<GuildId>,
}

#[test]
fn patch_rules() {
    assert_eq!(serde_json::to_string(&EditChannel::default()).unwrap(), "{}");
    assert_eq!(
        serde_json::to_string(&EditChannel { topic: Patch::Null, ..Default::default() }).unwrap(),
        r#"{"topic":null}"#
    );
    assert_eq!(
        serde_json::to_string(&EditChannel { topic: "x".to_owned().into(), ..Default::default() }).unwrap(),
        r#"{"topic":"x"}"#
    );
    let e: EditChannel = serde_json::from_str(r#"{"topic":null,"parent_id":"5"}"#).unwrap();
    assert_eq!(e.topic, Patch::Null);
    assert_eq!(e.parent_id, Patch::Value(Id::new(5)));
    let e: EditChannel = serde_json::from_str("{}").unwrap();
    assert!(e.topic.is_absent());
    assert_eq!(Patch::Value(1).map(|v| v + 1).into_option(), Some(2));
    assert!(Patch::<u8>::Null.is_null());
    #[derive(Serialize)]
    struct Hb {
        op: u8,
        d: Option<u64>,
    }
    assert_eq!(serde_json::to_string(&Hb { op: 1, d: None }).unwrap(), r#"{"op":1,"d":null}"#);
}

#[test]
fn union_rules() {
    let src = r#"{"type":1,"components":[{"type":2,"style":1,"custom_id":"a","label":"A","new_field":{"deep":[1,2,3]}},{"type":777,"mystery":{"a":1},"id":5}]}"#;
    let c: Component = serde_json::from_str(src).unwrap();
    let Component::ActionRow(ref row) = c else { panic!() };
    assert!(matches!(row.components[1], Component::Unknown { kind: 777, .. }));
    let out = serde_json::to_string(&c).unwrap();
    assert!(out.contains(r#"{"type":777,"mystery":{"a":1},"id":5}"#), "{out}");
    assert!(out.starts_with(r#"{"type":1,"components":[{"type":2,"style":1,"custom_id":"a","label":"A"}"#), "{out}");
    // tag not first, big tag, tag via Value
    let c: Component = serde_json::from_str(r#"{"content":"x","type":10}"#).unwrap();
    assert!(matches!(c, Component::TextDisplay(_)));
    let c: Component = serde_json::from_str(r#"{"type":4294967295}"#).unwrap();
    assert_eq!(c.kind(), 4_294_967_295);
    let c: Component = serde_json::from_value(json!({"type": 2, "style": 4})).unwrap();
    assert_eq!(serde_json::to_string(&c).unwrap(), r#"{"type":2,"style":4}"#);
    // missing tag and bad arm are errors, which the caller maps to DecodeFailed (level 2)
    assert!(serde_json::from_str::<Component>(r#"{"style":1}"#).is_err());
    assert!(serde_json::from_str::<Component>(r#"{"type":10,"content":{"o":1}}"#).is_err());
}

#[test]
fn union_under_flatten_fails_which_is_why_the_rule_exists() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Outer {
        #[serde(flatten)]
        inner: Inner,
    }
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Inner {
        c: Component,
    }
    assert!(serde_json::from_str::<Outer>(r#"{"c":{"type":10,"content":"x"}}"#).is_err());
}

#[test]
fn path_on_failure_works_through_union() {
    let good = r#"{"id":"1","author":{"id":"3"},"components":[{"type":10,"content":"hi"},{"type":777}]}"#;
    let m: Message = decode(good).unwrap();
    assert_eq!(m.components.len(), 2);
    let bad = r#"{"id":"1","author":{"id":"3"},"content":{"now":"object"}}"#;
    let e = decode::<Message>(bad).unwrap_err();
    assert_eq!(e.path, "content");
    let bad = r#"{"id":"1","author":{"id":"3"},"components":[{"type":10,"content":{"o":1}}]}"#;
    let e = decode::<Message>(bad).unwrap_err();
    assert!(e.path.starts_with("components"), "{}", e.path);
    // required-but-missing id
    assert!(decode::<Message>(r#"{"author":{"id":"3"}}"#).is_err());
    // nulls: null list is empty, unknown key ignored, referenced_message tri-state
    let m: Message =
        decode(r#"{"id":"1","author":{"id":"3"},"components":null,"future":[1],"referenced_message":null}"#).unwrap();
    assert!(m.components.is_empty());
    assert!(matches!(m.referenced_message, Patch::Null));
    let m: Message = decode(r#"{"id":"1","author":{"id":"3"}}"#).unwrap();
    assert!(m.referenced_message.is_absent());
}

#[test]
fn envelope_with_big_op_and_bad_body() {
    #[derive(Deserialize)]
    struct Envelope<'a> {
        op: u32,
        #[serde(default)]
        s: Option<u64>,
        #[serde(default, borrow)]
        t: Option<Cow<'a, str>>,
        #[serde(default, borrow)]
        d: Option<&'a RawValue>,
    }
    let f = r#"{"t":"MESSAGE_CREATE","s":42,"op":0,"d":{"id":"1","content":{"now":"an object"}}}"#;
    let e: Envelope<'_> = serde_json::from_str(f).unwrap();
    assert_eq!((e.op, e.s, e.t.as_deref()), (0, Some(42), Some("MESSAGE_CREATE")));
    assert!(decode::<Message>(e.d.unwrap().get()).is_err());
    let e: Envelope<'_> = serde_json::from_str(r#"{"op":999,"d":[]}"#).unwrap();
    assert_eq!(e.op, 999);
    let e: Envelope<'_> = serde_json::from_str(r#"{"t":"AB","op":0}"#).unwrap();
    assert_eq!(e.t.as_deref(), Some("AB"));
    let e: Envelope<'_> = serde_json::from_str(r#"{"op":11,"d":null}"#).unwrap();
    assert!(e.d.is_none(), "\"d\": null reads as None");
}

#[test]
fn numbers_lenient_timestamp() {
    #[derive(Deserialize)]
    struct N {
        #[serde(deserialize_with = "lenient_i64")]
        n: i64,
    }
    assert_eq!(serde_json::from_str::<N>(r#"{"n":1.0}"#).unwrap().n, 1);
    assert_eq!(serde_json::from_str::<N>(r#"{"n":5}"#).unwrap().n, 5);
    assert!(serde_json::from_str::<N>(r#"{"n":1.5}"#).is_err());
    assert!(serde_json::from_str::<i64>("1.0").is_err());
    let l: Lenient<User> = serde_json::from_str(r#"[{"id":"1"},{"id":{}},{"id":"3"}]"#).unwrap();
    assert_eq!((l.items.len(), l.dropped), (2, 1));
    for (s, want) in [
        ("2021-01-01T00:00:00+00:00", 1_609_459_200_000_000),
        ("2021-01-01T00:00:00.000000+00:00", 1_609_459_200_000_000),
        ("2021-01-01T00:00:00.5Z", 1_609_459_200_500_000),
        ("2021-01-01T01:00:00.123456+01:00", 1_609_459_200_123_456),
        ("1969-12-31T23:59:59.000000+00:00", -1_000_000),
    ] {
        let t = Timestamp::parse(s).unwrap_or_else(|| panic!("{s}"));
        assert_eq!(t.0, want, "{s}");
        let back = Timestamp::parse(&t.to_string()).unwrap();
        assert_eq!(back, t);
    }
    assert_eq!(Timestamp(1_609_459_200_123_456).to_string(), "2021-01-01T00:00:00.123456+00:00");
    assert_eq!(Timestamp(1_609_459_200_123_456).as_millis(), 1_609_459_200_123);
    for bad in [
        "nope",
        "",
        "2021-01-01T00:00:00",
        "2021-01-01T00:00:00.Z",
        "é021-01-01T00:00:00+00:00",
        "2021-01-01T00:00:00+0",
    ] {
        assert!(Timestamp::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn secret_redacts_but_serializes() {
    let s = Secret::new("hunter2");
    assert_eq!(format!("{s:?} {s}"), "Secret(***) ***");
    assert_eq!(s.expose(), "hunter2");
    assert_eq!(serde_json::to_string(&s).unwrap(), "\"hunter2\"");
    let back: Secret = serde_json::from_str("\"hunter2\"").unwrap();
    assert_eq!(back, s);
    assert_eq!(Secret::from("a"), Secret::from("a".to_owned()));
}

#[test]
fn bodies() {
    assert!(RequestBody::empty().json.is_none());
    assert!(().into_body().unwrap().json.is_none());
    let b = RequestBody::json(&json!({"a": 1})).unwrap();
    assert_eq!(b.json.unwrap().as_ref(), br#"{"a":1}"#);
    let v = json!({"b": 2}).into_body().unwrap();
    assert_eq!(v.json.unwrap().as_ref(), br#"{"b":2}"#);
    assert_eq!(ValidationError::new("a.b", "too long").to_string(), "a.b: too long");
}

#[test]
fn endpoints_default_to_discord() {
    let e = Endpoints::default();
    assert_eq!(e.api, "https://discord.com/api");
    assert!(e.gateway_override.is_none());
    let m = Endpoints::custom("http://127.0.0.1:1/api", Some("ws://127.0.0.1:2".to_owned()));
    assert_eq!(m.gateway_override.as_deref(), Some("ws://127.0.0.1:2"));
}
