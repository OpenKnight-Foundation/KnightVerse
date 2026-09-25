//! Real-time match-chat moderation (BE-76).
//!
//! Spectator chat is screened in [`crate::ws`] *before* it is published to the
//! Redis fan-out channel, so nothing unsanctioned reaches the other spectators
//! in a game. There are two layers:
//!
//! 1. **Local trie filter** ([`ProfanityFilter`]) — always on, pure CPU, no
//!    I/O. One pass over the message plus one trie walk per word keeps the
//!    whole review far inside the 5 ms budget the issue sets (see
//!    `filter_review_stays_inside_the_latency_budget`). Common profanity is
//!    masked in place with `***`; slurs reject the message outright.
//! 2. **Optional OpenAI moderation pass** ([`AiModerator`]) — enabled only
//!    when `OPENAI_API_KEY` is set. It runs *after* a message has been
//!    sanitized and published, on a background task, so it can never delay a
//!    broadcast. A flagged message still counts as an offence, which is what
//!    makes the layer useful: the local word list only has to be good, not
//!    perfect, because repeat offenders are muted by reputation.
//!
//! Muting ([`MuteTracker`]) is match-chat only and lasts 24 hours, per the
//! issue's acceptance criteria. Three offence points inside a ten minute
//! window mute the sender: a masked message is one point, a blocked message
//! (or an AI flag) is two.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// How long a repeat offender is kept out of match chat.
pub const MUTE_DURATION_SECS: u64 = 24 * 60 * 60;
/// Offences older than this are forgotten when weighing a mute.
pub const VIOLATION_WINDOW_SECS: u64 = 10 * 60;
/// Offence points inside the window that trigger a mute.
pub const MUTE_THRESHOLD: u32 = 3;
/// Points carried by a message that was masked and still published.
pub const POINTS_MASKED: u32 = 1;
/// Points carried by a message that was rejected (or flagged by the AI pass).
pub const POINTS_BLOCKED: u32 = 2;
/// What a violating word is replaced with.
pub const MASK: &str = "***";
/// Sweep the offender table once it grows past this many entries.
const OFFENDER_SWEEP_THRESHOLD: usize = 1024;

/// WebSocket error codes handed back to the sender of a moderated message.
/// These are chat-domain codes, far away from the HTTP-ish codes used
/// elsewhere in `ws.rs`.
pub const ERR_MESSAGE_BLOCKED: u16 = 4220;
pub const ERR_CHAT_MUTED: u16 = 4221;
pub const ERR_MESSAGE_MASKED: u16 = 4222;

/// Seconds since the Unix epoch, the clock every moderation decision uses.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------

/// How bad a listed word is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Published after the word is masked.
    Profanity,
    /// Rejected and reported; never broadcast.
    Slur,
}

/// How a listed word is matched against a word in the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// The word on its own (`ass` matches `ass`, but not `class` or `assume`).
    Exact,
    /// The word as the start of a word (`fuck` matches `fucking`, `fucker`).
    Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WordRule {
    severity: Severity,
    prefix: bool,
}

#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    word: Option<WordRule>,
}

/// A word found in a message, as a byte range into the original text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    start: usize,
    end: usize,
    folded: Vec<char>,
}

/// Outcome of screening one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterOutcome {
    /// The message as it should be broadcast: identical to the input when
    /// nothing matched, otherwise with every offending word replaced by
    /// [`MASK`].
    pub message: String,
    /// How many words were replaced.
    pub hits: usize,
    /// The worst thing found, if anything.
    pub severity: Option<Severity>,
}

impl FilterOutcome {
    fn clean(message: String) -> Self {
        Self {
            message,
            hits: 0,
            severity: None,
        }
    }
}

/// Trie-backed word filter.
///
/// Matching is deliberately conservative about the [Scunthorpe
/// problem](https://en.wikipedia.org/wiki/Scunthorpe_problem): short words are
/// matched whole (`ass` does not fire on `class`, `assume` or `assignment`),
/// and only deliberately long, unambiguous words are matched as prefixes.
/// Letters spread over single non-space separators are folded back together
/// (`f.u.c.k`, `n-i-g-g-e-r`), because that is the cheapest evasion there is.
#[derive(Debug, Clone, Default)]
pub struct ProfanityFilter {
    root: TrieNode,
    words: usize,
}

impl ProfanityFilter {
    /// An empty filter: everything passes.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a filter from `(word, severity, match)` triples.
    pub fn with_words<'a, I>(words: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, Severity, Match)>,
    {
        let mut filter = Self::default();
        for (word, severity, mode) in words {
            filter.insert(word, severity, mode);
        }
        filter
    }

    /// The default English word list, split between maskable profanity and
    /// slurs that reject the message.
    pub fn default_english() -> Self {
        Self::with_words(DEFAULT_WORDS.iter().copied())
    }

    /// Number of listed words.
    pub fn len(&self) -> usize {
        self.words
    }

    pub fn is_empty(&self) -> bool {
        self.words == 0
    }

    fn insert(&mut self, word: &str, severity: Severity, mode: Match) {
        let mut node = &mut self.root;
        let mut inserted = false;
        for c in word.chars() {
            let folded = fold(c).unwrap_or(c);
            node = node.children.entry(folded).or_default();
            inserted = true;
        }
        if inserted {
            node.word = Some(WordRule {
                severity,
                prefix: mode == Match::Prefix,
            });
            self.words += 1;
        }
    }

    /// Screen `text`, returning the message to broadcast and what was found.
    pub fn review(&self, text: &str) -> FilterOutcome {
        let tokens = tokenize(text);
        let mut message = String::with_capacity(text.len());
        let mut last = 0usize;
        let mut hits = 0usize;
        let mut worst: Option<Severity> = None;

        for token in &tokens {
            let Some(severity) = self.match_token(token) else {
                continue;
            };
            hits += 1;
            worst = Some(match worst {
                Some(Severity::Slur) => Severity::Slur,
                _ => severity,
            });
            message.push_str(&text[last..token.start]);
            message.push_str(MASK);
            last = token.end;
        }
        message.push_str(&text[last..]);

        if hits == 0 {
            return FilterOutcome::clean(text.to_string());
        }
        FilterOutcome {
            message,
            hits,
            severity: worst,
        }
    }

    /// Walk the trie from the start of `token`, returning the worst severity
    /// that matched.
    fn match_token(&self, token: &Token) -> Option<Severity> {
        let mut node = &self.root;
        let mut worst: Option<Severity> = None;

        for (index, c) in token.folded.iter().enumerate() {
            let Some(next) = node.children.get(c) else {
                break;
            };
            node = next;
            if let Some(rule) = node.word {
                let whole_word = index + 1 == token.folded.len();
                if whole_word || rule.prefix {
                    worst = Some(match worst {
                        Some(Severity::Slur) => Severity::Slur,
                        _ => rule.severity,
                    });
                }
            }
        }
        worst
    }
}

/// Fold a character to the letter it stands in for, or `None` when it is a
/// separator. Only ASCII is folded: homoglyph attacks that swap in other
/// alphabets are out of scope for a lightweight filter, and treating them as
/// separators keeps the false-positive rate at zero.
fn fold(c: char) -> Option<char> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c),
        '0' => Some('o'),
        '1' => Some('i'),
        '3' => Some('e'),
        '4' => Some('a'),
        '5' => Some('s'),
        '7' => Some('t'),
        '@' => Some('a'),
        '$' => Some('s'),
        '!' => Some('i'),
        '|' => Some('i'),
        _ => None,
    }
}

/// Split `text` into words. A word may contain single non-space separators
/// (`f.u.c.k` is one word), but a space or a run of separators always ends it,
/// so ordinary prose never gets glued together.
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut current: Option<Token> = None;
    // Separators seen since the last folded character: (count, saw_whitespace).
    let mut gap: Option<(usize, bool)> = None;

    for (index, c) in text.char_indices() {
        match fold(c) {
            Some(folded) => {
                let continues = match (current.is_some(), gap) {
                    (false, _) | (_, None) => true,
                    (true, Some((count, saw_space))) => count == 1 && !saw_space,
                };
                if !continues {
                    if let Some(token) = current.take() {
                        tokens.push(token);
                    }
                }
                let token = current.get_or_insert_with(|| Token {
                    start: index,
                    end: index,
                    folded: Vec::new(),
                });
                token.folded.push(folded);
                token.end = index + c.len_utf8();
                gap = None;
            }
            None => {
                let mut next = gap.unwrap_or((0, false));
                next.0 += 1;
                next.1 |= c.is_whitespace();
                gap = Some(next);
            }
        }
    }
    if let Some(token) = current.take() {
        tokens.push(token);
    }
    tokens
}

/// The default list. `Prefix` is reserved for words that have no innocent
/// longer form in English; everything else is matched whole, which is what
/// keeps `class`, `assume`, `assignment`, `dictionary`, `cocktail` and
/// `retardant` out of the filter's way.
const DEFAULT_WORDS: &[(&str, Severity, Match)] = &[
    // ---- slurs: the message is rejected ----------------------------------
    ("nigger", Severity::Slur, Match::Prefix),
    ("nigga", Severity::Slur, Match::Prefix),
    ("faggot", Severity::Slur, Match::Prefix),
    ("fag", Severity::Slur, Match::Exact),
    ("kike", Severity::Slur, Match::Prefix),
    ("spic", Severity::Slur, Match::Exact),
    ("chink", Severity::Slur, Match::Prefix),
    ("wetback", Severity::Slur, Match::Exact),
    ("tranny", Severity::Slur, Match::Prefix),
    ("retard", Severity::Slur, Match::Exact),
    ("retarded", Severity::Slur, Match::Exact),
    ("coon", Severity::Slur, Match::Exact),
    // ---- profanity: the message is masked and published ------------------
    ("fuck", Severity::Profanity, Match::Prefix),
    ("fck", Severity::Profanity, Match::Exact),
    ("fuk", Severity::Profanity, Match::Exact),
    ("shit", Severity::Profanity, Match::Prefix),
    ("bitch", Severity::Profanity, Match::Prefix),
    ("bastard", Severity::Profanity, Match::Prefix),
    ("cunt", Severity::Profanity, Match::Prefix),
    ("ass", Severity::Profanity, Match::Exact),
    ("asshole", Severity::Profanity, Match::Prefix),
    ("dumbass", Severity::Profanity, Match::Prefix),
    ("jackass", Severity::Profanity, Match::Prefix),
    ("dick", Severity::Profanity, Match::Exact),
    ("dickhead", Severity::Profanity, Match::Prefix),
    ("cock", Severity::Profanity, Match::Exact),
    ("cocksucker", Severity::Profanity, Match::Prefix),
    ("pussy", Severity::Profanity, Match::Prefix),
    ("slut", Severity::Profanity, Match::Prefix),
    ("whore", Severity::Profanity, Match::Prefix),
    ("wanker", Severity::Profanity, Match::Prefix),
    ("twat", Severity::Profanity, Match::Prefix),
    ("piss", Severity::Profanity, Match::Prefix),
    ("blowjob", Severity::Profanity, Match::Prefix),
    ("rape", Severity::Profanity, Match::Exact),
    ("raping", Severity::Profanity, Match::Exact),
    ("rapist", Severity::Profanity, Match::Exact),
    ("spaz", Severity::Profanity, Match::Exact),
    ("moron", Severity::Profanity, Match::Prefix),
    ("idiot", Severity::Profanity, Match::Prefix),
];

// ---------------------------------------------------------------------------
// Muting
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
struct Offender {
    points: u32,
    offences: u32,
    window_started_at: u64,
    muted_until: u64,
}

/// What is known about one chatter right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    pub points: u32,
    pub offences: u32,
    pub muted_until: u64,
}

/// Per-user offence ledger. In-memory on purpose: match chat is ephemeral, a
/// restart of the backend clearing recent offences is acceptable, and it keeps
/// the hot path free of Redis round-trips.
#[derive(Debug, Default)]
struct MuteTracker {
    offenders: HashMap<String, Offender>,
}

impl MuteTracker {
    /// The timestamp a user's mute ends at, if they are muted right now.
    fn mute_until(&mut self, user: &str, now: u64) -> Option<u64> {
        let offender = self.offenders.get(user)?;
        (offender.muted_until > now).then_some(offender.muted_until)
    }

    /// Record `points` against `user`, returning the mute end timestamp when
    /// this offence is the one that triggers (or extends) a mute.
    fn record(&mut self, user: &str, points: u32, now: u64) -> Option<u64> {
        if !self.offenders.contains_key(user) && self.offenders.len() >= OFFENDER_SWEEP_THRESHOLD {
            self.clear_expired(now);
        }
        let offender = self.offenders.entry(user.to_string()).or_default();
        if now.saturating_sub(offender.window_started_at) > VIOLATION_WINDOW_SECS {
            offender.points = 0;
            offender.window_started_at = now;
        }
        offender.points = offender.points.saturating_add(points);
        offender.offences = offender.offences.saturating_add(1);

        if offender.points >= MUTE_THRESHOLD {
            let until = now.saturating_add(MUTE_DURATION_SECS);
            offender.muted_until = offender.muted_until.max(until);
            return Some(offender.muted_until);
        }
        None
    }

    fn standing(&self, user: &str, now: u64) -> Standing {
        match self.offenders.get(user) {
            Some(offender) => {
                let window_live =
                    now.saturating_sub(offender.window_started_at) <= VIOLATION_WINDOW_SECS;
                Standing {
                    points: if window_live { offender.points } else { 0 },
                    offences: offender.offences,
                    muted_until: offender.muted_until,
                }
            }
            None => Standing::default(),
        }
    }

    /// Drop offenders with neither a live mute nor a live offence window.
    fn clear_expired(&mut self, now: u64) {
        self.offenders.retain(|_, offender| {
            offender.muted_until > now
                || now.saturating_sub(offender.window_started_at) <= VIOLATION_WINDOW_SECS
        });
    }
}

// ---------------------------------------------------------------------------
// Moderator
// ---------------------------------------------------------------------------

/// What the chat handler should do with a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatDecision {
    /// Broadcast this (possibly masked) message.
    Publish { message: String, masked: usize },
    /// Drop the message and warn the sender.
    Blocked { code: u16, reason: String },
    /// The sender is muted in match chat; drop the message and warn them.
    Muted { code: u16, reason: String, until: u64 },
}

impl ChatDecision {
    /// The warning to hand back to the sender, if any.
    pub fn warning(&self) -> Option<(u16, String)> {
        match self {
            ChatDecision::Publish { masked, .. } if *masked > 0 => {
                Some((ERR_MESSAGE_MASKED, masked_warning(*masked)))
            }
            ChatDecision::Publish { .. } => None,
            ChatDecision::Blocked { code, reason } => Some((*code, reason.clone())),
            ChatDecision::Muted { code, reason, .. } => Some((*code, reason.clone())),
        }
    }
}

/// The chat moderation service: word filter plus mute ledger, optionally with
/// an OpenAI pass layered on top.
#[derive(Debug, Clone)]
pub struct ChatModerator {
    filter: ProfanityFilter,
    tracker: Arc<Mutex<MuteTracker>>,
    ai: Option<AiModerator>,
}

impl Default for ChatModerator {
    fn default() -> Self {
        Self::with_default_words()
    }
}

impl ChatModerator {
    pub fn new(filter: ProfanityFilter) -> Self {
        Self {
            filter,
            tracker: Arc::new(Mutex::new(MuteTracker::default())),
            ai: None,
        }
    }

    /// The moderator every process uses by default: the built-in word list,
    /// plus the OpenAI pass when `OPENAI_API_KEY` is configured.
    pub fn with_default_words() -> Self {
        Self::new(ProfanityFilter::default_english())
    }

    pub fn from_env() -> Self {
        Self::with_default_words().with_ai(AiModerator::from_env())
    }

    /// Process-wide instance, so every WebSocket session shares one mute
    /// table.
    pub fn global() -> Self {
        static GLOBAL: OnceLock<ChatModerator> = OnceLock::new();
        GLOBAL.get_or_init(ChatModerator::from_env).clone()
    }

    pub fn with_ai(mut self, ai: Option<AiModerator>) -> Self {
        self.ai = ai;
        self
    }

    pub fn ai(&self) -> Option<&AiModerator> {
        self.ai.as_ref()
    }

    pub fn filter(&self) -> &ProfanityFilter {
        &self.filter
    }

    /// Is `user` muted in match chat as of `now`?
    pub fn is_muted(&self, user: &str, now: u64) -> Option<u64> {
        self.lock().mute_until(user, now)
    }

    /// Offence standing for `user`, for tests and diagnostics.
    pub fn standing(&self, user: &str, now: u64) -> Standing {
        self.lock().standing(user, now)
    }

    /// Screen an inbound chat message. `now` is Unix seconds.
    ///
    /// A clean message is published untouched. A message containing profanity
    /// is published with those words masked, and counts one offence. A message
    /// containing a slur is rejected outright and counts two. Whatever pushes
    /// a user over [`MUTE_THRESHOLD`] inside [`VIOLATION_WINDOW_SECS`] is
    /// itself rejected — the mute starts with the message that earned it.
    pub fn review(&self, user: &str, message: &str, now: u64) -> ChatDecision {
        if let Some(until) = self.is_muted(user, now) {
            return ChatDecision::Muted {
                code: ERR_CHAT_MUTED,
                reason: muted_warning(until.saturating_sub(now)),
                until,
            };
        }

        let outcome = self.filter.review(message);
        match outcome.severity {
            None => ChatDecision::Publish {
                message: outcome.message,
                masked: 0,
            },
            Some(Severity::Slur) => {
                let until = self.record(user, POINTS_BLOCKED, now);
                match until {
                    Some(until) => ChatDecision::Muted {
                        code: ERR_CHAT_MUTED,
                        reason: muted_warning(until.saturating_sub(now)),
                        until,
                    },
                    None => ChatDecision::Blocked {
                        code: ERR_MESSAGE_BLOCKED,
                        reason: blocked_warning(),
                    },
                }
            }
            Some(Severity::Profanity) => {
                let until = self.record(user, POINTS_MASKED, now);
                match until {
                    Some(until) => ChatDecision::Muted {
                        code: ERR_CHAT_MUTED,
                        reason: muted_warning(until.saturating_sub(now)),
                        until,
                    },
                    None => ChatDecision::Publish {
                        message: outcome.message,
                        masked: outcome.hits,
                    },
                }
            }
        }
    }

    /// Record the outcome of the async OpenAI pass. Returns the mute end
    /// timestamp when the flag is what triggers a mute.
    ///
    /// The message has already been broadcast by the time this runs, so this
    /// is reputation bookkeeping: it stops a determined evader from keeping it
    /// up, rather than unsending what was said.
    pub fn record_ai_flag(&self, user: &str, now: u64) -> Option<u64> {
        self.record(user, POINTS_BLOCKED, now)
    }

    fn record(&self, user: &str, points: u32, now: u64) -> Option<u64> {
        self.lock().record(user, points, now)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MuteTracker> {
        self.tracker.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Warning shown to the sender when a masked message was published.
pub fn masked_warning(masked: usize) -> String {
    format!(
        "Your message was filtered: {masked} word(s) violated the chat rules and were replaced with {MASK}."
    )
}

/// Warning shown to the sender when a message was rejected outright.
pub fn blocked_warning() -> String {
    "Your message was blocked: match chat does not allow slurs or harassment.".to_string()
}

/// Warning shown to a muted sender.
pub fn muted_warning(remaining_secs: u64) -> String {
    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;
    format!(
        "You are muted in match chat for repeated violations. Try again in {hours}h {minutes}m."
    )
}

// ---------------------------------------------------------------------------
// Optional OpenAI moderation pass
// ---------------------------------------------------------------------------

/// OpenAI's omni moderation model, which covers text and image input.
pub const DEFAULT_AI_MODEL: &str = "omni-moderation-latest";
pub const DEFAULT_AI_ENDPOINT: &str = "https://api.openai.com/v1/moderations";
/// The AI pass is best-effort: it must never hold a chat message hostage.
pub const AI_TIMEOUT: Duration = Duration::from_secs(3);

/// Verdict of one OpenAI moderation call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiVerdict {
    pub flagged: bool,
    /// Names of the categories the model flagged.
    pub categories: Vec<String>,
    /// Highest category score reported (0.0 when the response omitted scores).
    pub max_score: f32,
}

/// Thin, best-effort client for the OpenAI moderation endpoint.
#[derive(Debug, Clone)]
pub struct AiModerator {
    client: reqwest::Client,
    api_key: String,
    model: String,
    endpoint: String,
}

impl AiModerator {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(AI_TIMEOUT)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            api_key: api_key.into(),
            model: DEFAULT_AI_MODEL.to_string(),
            endpoint: DEFAULT_AI_ENDPOINT.to_string(),
        }
    }

    /// Build the client from the environment, or `None` when no key is set —
    /// which is what keeps the pass off in tests and local runs.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }
        let mut moderator = Self::new(api_key);
        if let Ok(model) = std::env::var("OPENAI_MODERATION_MODEL") {
            if !model.trim().is_empty() {
                moderator.model = model;
            }
        }
        if let Ok(endpoint) = std::env::var("OPENAI_MODERATION_URL") {
            if !endpoint.trim().is_empty() {
                moderator.endpoint = endpoint;
            }
        }
        Some(moderator)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Request body for one moderation call.
    pub fn request_body(&self, text: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "input": text,
        })
    }

    /// Parse a moderation response. Kept pure so the mapping from OpenAI's
    /// schema to [`AiVerdict`] is testable without network access.
    pub fn parse_verdict(body: &serde_json::Value) -> Result<AiVerdict, String> {
        let result = body
            .get("results")
            .and_then(|results| results.get(0))
            .ok_or_else(|| "moderation response has no results[0]".to_string())?;

        let flagged = result
            .get("flagged")
            .and_then(|flagged| flagged.as_bool())
            .unwrap_or(false);

        let categories = result
            .get("categories")
            .and_then(|categories| categories.as_object())
            .map(|categories| {
                categories
                    .iter()
                    .filter(|(_, flagged)| flagged.as_bool().unwrap_or(false))
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let max_score = result
            .get("category_scores")
            .and_then(|scores| scores.as_object())
            .map(|scores| {
                scores
                    .values()
                    .filter_map(|score| score.as_f64())
                    .fold(0.0f64, f64::max) as f32
            })
            .unwrap_or(0.0);

        Ok(AiVerdict {
            flagged,
            categories,
            max_score,
        })
    }

    /// Screen `text`. Errors are returned rather than retried: the caller
    /// logs and moves on, because chat must not wait on a third party.
    pub async fn check(&self, text: &str) -> Result<AiVerdict, String> {
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&self.request_body(text))
            .send()
            .await
            .map_err(|e| format!("moderation request failed: {e}"))?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("moderation response was not JSON: {e}"))?;

        if !status.is_success() {
            return Err(format!("moderation request rejected with {status}: {body}"));
        }
        Self::parse_verdict(&body)
    }
}

impl ChatModerator {
    /// Run the optional AI pass for a message that has already been published.
    /// Failures are logged at debug level and otherwise ignored.
    pub async fn audit(&self, user: &str, text: &str) -> Option<u64> {
        let ai = self.ai.as_ref()?;
        match ai.check(text).await {
            Ok(verdict) if verdict.flagged => {
                debug!(
                    "OpenAI moderation flagged a message from {} (categories: {:?}, score {:.2})",
                    user, verdict.categories, verdict.max_score
                );
                let until = self.record_ai_flag(user, unix_now());
                if let Some(until) = until {
                    warn!("Muted {} in match chat until {}", user, until);
                }
                until
            }
            Ok(_) => None,
            Err(e) => {
                debug!("OpenAI moderation pass skipped: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn filter() -> ProfanityFilter {
        ProfanityFilter::default_english()
    }

    fn moderator() -> ChatModerator {
        ChatModerator::new(filter())
    }

    // ---- sanitizer ------------------------------------------------------

    #[test]
    fn clean_messages_pass_through_untouched() {
        let filter = filter();
        for message in [
            "gg wp",
            "nice knight fork!",
            "anyone want a rematch?",
            "that was a brilliant endgame",
            "e4 e5 Nf3 Nc6 Bb5",
        ] {
            let outcome = filter.review(message);
            assert_eq!(outcome.message, message);
            assert_eq!(outcome.hits, 0);
            assert_eq!(outcome.severity, None);
        }
    }

    #[test]
    fn innocent_words_containing_profanity_are_left_alone() {
        let filter = filter();
        // The Scunthorpe set: every one of these contains a listed word.
        for message in [
            "classic Sicilian",
            "the grass is green",
            "assignment submitted",
            "i assume you saw that",
            "assess the position",
            "dictionary attack",
            "cocktail hour",
            "fire retardant",
            "shipment of chess sets",
            "hello Cassandra",
        ] {
            let outcome = filter.review(message);
            assert_eq!(outcome.message, message, "false positive on {message:?}");
            assert_eq!(outcome.hits, 0, "false positive on {message:?}");
        }
    }

    #[test]
    fn profanity_is_masked_in_place() {
        let outcome = filter().review("you fucking idiot, gg");
        assert_eq!(outcome.message, "you *** ***, gg");
        assert_eq!(outcome.hits, 2);
        assert_eq!(outcome.severity, Some(Severity::Profanity));
    }

    #[test]
    fn inflected_profanity_is_masked_too() {
        let outcome = filter().review("stop bitching and shitting on my moves");
        assert_eq!(outcome.message, "stop *** and *** on my moves");
        assert_eq!(outcome.hits, 2);
    }

    #[test]
    fn slurs_are_reported_as_severe() {
        let outcome = filter().review("you are a nigger");
        assert_eq!(outcome.message, "you are a ***");
        assert_eq!(outcome.severity, Some(Severity::Slur));

        // A slur plus profanity is still severe.
        let mixed = filter().review("shut up you faggot shithead");
        assert_eq!(mixed.severity, Some(Severity::Slur));
    }

    #[test]
    fn separator_evasion_is_still_caught() {
        let filter = filter();
        for message in ["f.u.c.k", "f-u-c-k you", "s.h.i.t play", "n.i.g.g.e.r"] {
            let outcome = filter.review(message);
            assert!(outcome.hits > 0, "evasion missed: {message:?}");
            assert!(outcome.message.contains(MASK));
        }
    }

    #[test]
    fn repeated_separators_do_not_glue_words_together() {
        // Two separators is a gap, not a word: this is what keeps
        // "gg -- wp" and "e4.. c5" from becoming one giant word.
        let outcome = filter().review("gg -- wp");
        assert_eq!(outcome.hits, 0);
        assert_eq!(outcome.message, "gg -- wp");
    }

    #[test]
    fn leetspeak_digits_and_symbols_are_folded() {
        let filter = filter();
        for message in ["sh1t", "b1tch", "a$$hole", "@sshole"] {
            let outcome = filter.review(message);
            assert!(outcome.hits > 0, "leet evasion missed: {message:?}");
        }
    }

    #[test]
    fn masking_preserves_surrounding_text_and_punctuation() {
        let outcome = filter().review("wow, what a shit move: e4!!");
        assert_eq!(outcome.message, "wow, what a *** move: e4!!");
        assert_eq!(outcome.hits, 1);
    }

    #[test]
    fn custom_word_lists_are_honoured() {
        let filter = ProfanityFilter::with_words([("zugzwang", Severity::Profanity, Match::Exact)]);
        assert_eq!(filter.len(), 1);
        let outcome = filter.review("what a zugzwang");
        assert_eq!(outcome.message, "what a ***");
        assert_eq!(outcome.hits, 1);
        // The default list is not implied by a custom one.
        assert_eq!(filter.review("what a shit move").hits, 0);
    }

    #[test]
    fn filter_review_stays_inside_the_latency_budget() {
        // Acceptance criterion: sub-5 ms local filtering. A realistic chat
        // message is screened thousands of times over below; the assertion is
        // the per-message average, so a single slow scheduler tick cannot
        // fail the build.
        let filter = filter();
        let message = "that was a brilliant knight sacrifice, but you fucking missed mate in two";
        const ITERATIONS: u32 = 5_000;

        // Warm up the trie before timing.
        assert!(filter.review(message).hits > 0);

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            std::hint::black_box(filter.review(message));
        }
        let average = start.elapsed().as_secs_f64() / f64::from(ITERATIONS) * 1_000.0;
        assert!(
            average < 5.0,
            "average review took {average:.3} ms, budget is 5 ms"
        );
    }

    // ---- muting ---------------------------------------------------------

    #[test]
    fn clean_chat_never_mutes() {
        let moderator = moderator();
        let mut now = 1_000;
        for _ in 0..50 {
            let decision = moderator.review("alice", "gg wp", now);
            assert!(matches!(decision, ChatDecision::Publish { masked: 0, .. }));
            now += 30;
        }
        assert_eq!(moderator.is_muted("alice", now), None);
        assert_eq!(moderator.standing("alice", now).offences, 0);
    }

    #[test]
    fn single_violation_is_masked_but_not_muted() {
        let moderator = moderator();
        let decision = moderator.review("bob", "what a shit move", 1_000);
        match decision {
            ChatDecision::Publish { message, masked } => {
                assert_eq!(message, "what a *** move");
                assert_eq!(masked, 1);
            }
            other => panic!("expected publish, got {other:?}"),
        }
        assert_eq!(moderator.is_muted("bob", 1_000), None);
        assert_eq!(moderator.standing("bob", 1_000).points, POINTS_MASKED);
    }

    #[test]
    fn repeat_offender_is_muted_for_24_hours() {
        let moderator = moderator();
        let now = 10_000;

        // First two offences are masked and published.
        for _ in 0..(MUTE_THRESHOLD - 1) {
            assert!(matches!(
                moderator.review("carol", "shit move", now),
                ChatDecision::Publish { .. }
            ));
        }

        // The third offence trips the threshold: it is rejected, not published.
        let decision = moderator.review("carol", "shit move", now);
        let until = match decision {
            ChatDecision::Muted { until, reason, code } => {
                assert_eq!(code, ERR_CHAT_MUTED);
                assert!(reason.contains("24h"), "unhelpful warning: {reason}");
                until
            }
            other => panic!("expected mute, got {other:?}"),
        };
        assert_eq!(until, now + MUTE_DURATION_SECS);

        // Everything they send for the next 24 hours is dropped, even clean chat.
        let muted = moderator.review("carol", "gg wp", now + 60);
        assert!(matches!(muted, ChatDecision::Muted { .. }));
        assert_eq!(moderator.is_muted("carol", now + 60), Some(until));

        // The mute lapses after exactly 24 hours.
        assert_eq!(moderator.is_muted("carol", until), None);
        let after = moderator.review("carol", "gg wp", until + 1);
        assert!(matches!(after, ChatDecision::Publish { masked: 0, .. }));
    }

    #[test]
    fn slurs_are_rejected_and_count_double() {
        let moderator = moderator();
        let now = 5_000;
        let decision = moderator.review("dave", "you nigger", now);
        match decision {
            ChatDecision::Blocked { code, .. } => assert_eq!(code, ERR_MESSAGE_BLOCKED),
            other => panic!("expected block, got {other:?}"),
        }
        assert_eq!(moderator.standing("dave", now).points, POINTS_BLOCKED);
        assert_eq!(moderator.is_muted("dave", now), None);

        // A second slur reaches the threshold and mutes on the spot.
        let second = moderator.review("dave", "nigger", now + 1);
        assert!(matches!(second, ChatDecision::Muted { .. }));
    }

    #[test]
    fn offences_expire_outside_the_window() {
        let moderator = moderator();
        let start = 100_000;
        for _ in 0..(MUTE_THRESHOLD - 1) {
            moderator.review("erin", "shit", start);
        }
        // Well past the window: the earlier offences no longer count.
        let later = start + VIOLATION_WINDOW_SECS + 1;
        let decision = moderator.review("erin", "shit", later);
        assert!(
            matches!(decision, ChatDecision::Publish { .. }),
            "offences should have expired, got {decision:?}"
        );
        assert_eq!(moderator.standing("erin", later).points, POINTS_MASKED);
    }

    #[test]
    fn mutes_are_per_user_and_in_memory() {
        let moderator = moderator();
        let now = 1;
        for _ in 0..MUTE_THRESHOLD {
            moderator.review("frank", "shit", now);
        }
        assert!(moderator.is_muted("frank", now).is_some());
        assert_eq!(moderator.is_muted("grace", now), None);

        // Clones share the ledger, which is what makes one moderator per
        // process enough.
        let clone = moderator.clone();
        assert_eq!(clone.is_muted("frank", now), moderator.is_muted("frank", now));
    }

    #[test]
    fn expired_offenders_are_swept_out() {
        let mut tracker = MuteTracker::default();
        tracker.record("old", POINTS_MASKED, 1_000);
        assert_eq!(tracker.offenders.len(), 1);
        tracker.clear_expired(1_000 + VIOLATION_WINDOW_SECS + 1);
        assert!(tracker.offenders.is_empty());
    }

    #[test]
    fn decisions_map_to_sender_warnings() {
        let moderator = moderator();
        let now = 42;

        let clean = moderator.review("harry", "gg", now);
        assert_eq!(clean.warning(), None);

        let masked = moderator.review("harry", "shit", now);
        let (code, warning) = masked.warning().expect("masked messages warn the sender");
        assert_eq!(code, ERR_MESSAGE_MASKED);
        assert!(warning.contains(MASK));

        // A fresh user, so the slur only blocks this message instead of
        // pushing them over the mute threshold and returning ERR_CHAT_MUTED.
        let blocked = moderator.review("ivan", "faggot", now);
        let (code, warning) = blocked.warning().expect("blocked messages warn the sender");
        assert_eq!(code, ERR_MESSAGE_BLOCKED);
        assert!(warning.contains("blocked"));
    }

    // ---- optional AI pass ------------------------------------------------

    #[test]
    fn ai_pass_is_off_without_a_key() {
        std::env::remove_var("OPENAI_API_KEY");
        assert!(AiModerator::from_env().is_none());
        assert!(ChatModerator::with_default_words().ai().is_none());
    }

    #[test]
    fn ai_request_body_carries_model_and_input() {
        let ai = AiModerator::new("test-key");
        let body = ai.request_body("hello");
        assert_eq!(body["model"], DEFAULT_AI_MODEL);
        assert_eq!(body["input"], "hello");
    }

    #[test]
    fn ai_verdict_parses_flagged_categories_and_scores() {
        let body = serde_json::json!({
            "id": "modr-1",
            "model": DEFAULT_AI_MODEL,
            "results": [{
                "flagged": true,
                "categories": { "harassment": true, "hate": false, "violence": true },
                "category_scores": { "harassment": 0.72, "hate": 0.01, "violence": 0.93 }
            }]
        });
        let verdict = AiModerator::parse_verdict(&body).expect("valid response");
        assert!(verdict.flagged);
        assert_eq!(verdict.categories, vec!["harassment", "violence"]);
        assert!((verdict.max_score - 0.93).abs() < f32::EPSILON);
    }

    #[test]
    fn ai_verdict_rejects_a_response_without_results() {
        let body = serde_json::json!({ "error": { "message": "bad key" } });
        assert!(AiModerator::parse_verdict(&body).is_err());
    }

    #[test]
    fn ai_flagged_messages_count_towards_a_mute() {
        let moderator = moderator().with_ai(Some(AiModerator::new("test-key")));
        let now = 7;

        // The AI pass runs after publication, so it only ever records points.
        assert_eq!(moderator.record_ai_flag("ivan", now), None);
        assert_eq!(moderator.standing("ivan", now).points, POINTS_BLOCKED);
        assert!(moderator.record_ai_flag("ivan", now).is_some());
    }
}
