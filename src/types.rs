use std::{borrow::Cow, ops::Deref, sync::Arc};

use druid::{im, widget::ListIter, Data, Selector};
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher as _};

#[bitflags(default = Select | WindowClosed)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Subscription {
    Select        = 0b0001,
    CursorMove    = 0b0010,
    InputChange   = 0b0100,
    WindowClosed  = 0b1000,
}

#[derive(Debug)]
pub enum Event {
    Select(Option<usize>),
    CursorMove(usize),
    InputChange(String),
    WindowClosed,
}

impl Event {
    #[must_use]
    pub fn needed(&self, subscription: BitFlags<Subscription>) -> bool {
        match self {
            Event::Select(_) => subscription.contains(Subscription::Select),
            Event::CursorMove(_) => subscription.contains(Subscription::CursorMove),
            Event::InputChange(_) => subscription.contains(Subscription::InputChange),
            Event::WindowClosed => subscription.contains(Subscription::WindowClosed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Data, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Matcher {
    None,
    Fuzzy,
}

impl Default for Matcher {
    fn default() -> Self {
        Self::Fuzzy
    }
}

#[derive(Debug, Deserialize)]
pub struct Registration {
    pub protocol_version: u8,
    #[serde(default)]
    pub subscribe_to: BitFlags<Subscription>,
    #[serde(default)]
    pub matcher: Matcher,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "key", content = "data")]
pub enum ServerEvent {
    Busy,
    Registered(usize),
    ServerTooOld(u8),
    Select(Option<usize>),
    CursorMove(usize),
    InputChange(String),
    WindowClosed,
}

impl From<Event> for ServerEvent {
    fn from(ui_event: Event) -> Self {
        match ui_event {
            Event::Select(n) => ServerEvent::Select(n),
            Event::CursorMove(n) => ServerEvent::CursorMove(n),
            Event::InputChange(input) => ServerEvent::InputChange(input),
            Event::WindowClosed => ServerEvent::WindowClosed,
        }
    }
}

#[derive(Debug, Clone, Data, PartialOrd, Ord, PartialEq, Eq)]
pub struct ArcStr(Arc<str>);

impl<'de> Deserialize<'de> for ArcStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        Ok(Self(Arc::from(s)))
    }
}

impl Deref for ArcStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

#[derive(Debug, Clone, Data, PartialOrd, Ord, PartialEq, Eq, Deserialize)]
pub struct Choice {
    #[serde(default)]
    pub priority: i64,
    pub id: usize,
    pub text: ArcStr,
}

#[derive(Debug, Default, Clone, Data, Deserialize)]
pub struct ChoiceSet {
    pub options: im::OrdSet<Choice>,
    #[serde(default)]
    pub selected: Option<usize>,
}

impl ChoiceSet {
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.options.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn fuzzy_sort(&mut self, input: &str) {
        take_mut::take(&mut self.options, |options| {
            let matcher = SkimMatcherV2::default();
            options
                .into_iter()
                .map(|mut option| {
                    option.priority = -matcher
                        .fuzzy_match(&*option.text, input)
                        .unwrap_or(i64::MIN + 1);
                    option
                })
                .collect()
        });
    }
}

#[derive(Debug, Clone, Data)]
pub struct Indices {
    pub current: usize,
    pub selected: Option<usize>,
}

impl Indices {
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
            .map_or(false, |selected| self.current == selected)
    }
}

impl ListIter<(Indices, Choice)> for ChoiceSet {
    fn for_each(&self, mut cb: impl FnMut(&(Indices, Choice), usize)) {
        let selected = self.selected;
        for (idx, item) in self.options.iter().enumerate() {
            cb(
                &(
                    Indices {
                        current: idx,
                        selected,
                    },
                    item.clone(),
                ),
                idx,
            );
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut (Indices, Choice), usize)) {
        let selected = self.selected;
        for (idx, item) in self.options.iter().enumerate() {
            cb(
                &mut (
                    Indices {
                        current: idx,
                        selected,
                    },
                    item.clone(),
                ),
                idx,
            );
        }
    }

    #[inline]
    fn data_len(&self) -> usize {
        self.len()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "key", content = "data", rename_all = "snake_case")]
pub enum ClientRequest {
    Stop,
    SetChoices(ChoiceSet),
    SetInput(String),
}

pub const CLIENT_REQUEST_SELECTOR: Selector<ClientRequest> = Selector::new("ClientRequest");
