//! Canonical, source-agnostic match model.
//!
//! Coordinates: metres on a 105 x 68 pitch, always from the perspective of the
//! team performing the action: x = 0 own goal line, x = 105 opponent goal line,
//! y = 0 right touchline, y = 68 left touchline (Opta convention).

use serde::{Deserialize, Serialize};

pub const PITCH_L: f64 = 105.0;
pub const PITCH_W: f64 = 68.0;
pub const GOAL_X: f64 = 105.0;
pub const GOAL_Y: f64 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Home,
    Away,
}
impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Home => Side::Away,
            Side::Away => Side::Home,
        }
    }
    pub fn idx(self) -> usize {
        match self {
            Side::Home => 0,
            Side::Away => 1,
        }
    }
    pub fn both() -> [Side; 2] {
        [Side::Home, Side::Away]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Pass,
    Carry,
    TakeOn,
    Shot,
    Tackle,
    Interception,
    Recovery,
    Clearance,
    Block,
    Foul,
    Aerial,
    Save,
    Claim,
    Sweeper,
    Dispossessed,
    BadTouch,
    Error,
    Challenge,
    BallTouch,
    Card,
    SubOff,
    SubOn,
    Other,
}

impl Kind {
    /// Actions that indicate the acting team controls the ball.
    pub fn controls_ball(self) -> bool {
        matches!(
            self,
            Kind::Pass | Kind::Carry | Kind::TakeOn | Kind::Shot | Kind::Recovery | Kind::Claim
        )
    }
    pub fn is_on_ball(self) -> bool {
        !matches!(self, Kind::Card | Kind::SubOff | Kind::SubOn | Kind::Other)
    }
    pub fn is_def_action(self) -> bool {
        matches!(
            self,
            Kind::Tackle | Kind::Interception | Kind::Challenge | Kind::Foul | Kind::Block | Kind::Clearance
        )
    }
    pub fn is_ball_win(self) -> bool {
        matches!(self, Kind::Tackle | Kind::Interception | Kind::Recovery)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Restart {
    Corner,
    FreeKick,
    ThrowIn,
    GoalKick,
    KickOff,
    Penalty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Body {
    Right,
    Left,
    Head,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShotOutcome {
    Goal,
    Saved,
    OffTarget,
    Woodwork,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Handling {
    Held,
    ParriedSafe,
    ParriedDanger,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassInfo {
    pub cross: bool,
    pub through: bool,
    pub key_pass: bool,
    pub assist: bool,
    pub long: bool,
    pub launch: bool,
    pub head: bool,
    pub cutback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShotInfo {
    pub outcome: ShotOutcome,
    pub body: Body,
    pub first_time: bool,
    pub big_chance: bool,
    pub penalty: bool,
    pub own_goal: bool,
    pub direct_free_kick: bool,
    pub fast_break: bool,
    /// Goal-mouth position: u = -1 right post .. +1 left post (from shooter), z = 0 ground .. 1 crossbar.
    pub goal_u: Option<f64>,
    pub goal_z: Option<f64>,
    pub assist_cross: bool,
    pub assist_through: bool,
    pub assist_cutback: bool,
    /// External model values (FotMob, SofaScore or StatsBomb) kept for comparison.
    pub xg_ext: Option<f64>,
    pub xgot_ext: Option<f64>,
    /// In-house values filled by the model layer.
    pub xg: f64,
    pub psxg: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub period: u8,
    pub minute: u32,
    pub second: u32,
    /// Seconds on an expanded clock (all periods back to back incl. added time).
    pub t: f64,
    pub side: Side,
    pub player: Option<u64>,
    pub kind: Kind,
    pub success: bool,
    pub x: f64,
    pub y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub restart: Option<Restart>,
    pub pass: Option<PassInfo>,
    pub shot: Option<ShotInfo>,
    pub handling: Option<Handling>,
    pub inferred: bool,
    pub related: Option<u64>,
    pub possession: u32,
    pub sequence: u32,
    pub related_player: Option<u64>,
}

impl Event {
    pub fn label_minute(&self) -> String {
        format!("{}'", self.minute + 1)
    }
    pub fn end(&self) -> (f64, f64) {
        (self.end_x.unwrap_or(self.x), self.end_y.unwrap_or(self.y))
    }
    pub fn is_open_play(&self) -> bool {
        self.restart.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub name: String,
    pub short: String,
    pub shirt: u32,
    pub position: String,
    pub role: Role,
    pub starter: bool,
    pub age: Option<f64>,
    pub on_t: f64,
    pub off_t: Option<f64>,
    pub replaced: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Goalkeeper,
    Defender,
    Midfielder,
    Forward,
}
impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Goalkeeper => "Goalkeeper",
            Role::Defender => "Defender",
            Role::Midfielder => "Midfielder",
            Role::Forward => "Forward",
        }
    }
    pub fn from_position(p: &str) -> Role {
        let p = p.to_uppercase();
        if p.contains("MIDFIELD") {
            return Role::Midfielder;
        }
        if p.contains("WING BACK") {
            return Role::Defender;
        }
        if p == "GK" || p.contains("GOALKEEPER") {
            Role::Goalkeeper
        } else if p.starts_with("FW") || p.contains("FORWARD") || p.contains("STRIKER") || p.contains("WING") && !p.contains("BACK") || p == "CF" || p == "ST" || p == "LW" || p == "RW" || p == "SS" {
            Role::Forward
        } else if p.starts_with('D') && !p.starts_with("DM") || p.contains("BACK") || p.contains("DEFEN") || p == "CB" || p == "LB" || p == "RB" || p == "LWB" || p == "RWB" {
            Role::Defender
        } else {
            Role::Midfielder
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationSpell {
    pub name: String,
    pub start_t: f64,
    pub end_t: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub source_id: String,
    pub name: String,
    pub short_name: String,
    pub manager: Option<String>,
    pub players: Vec<Player>,
    pub formations: Vec<FormationSpell>,
    pub score: u32,
    /// Colour hints captured from sources (hex), resolved later by theme.rs.
    pub colour_hints: Vec<(String, String)>,
    pub crest_url: Option<String>,
}

impl Team {
    pub fn player(&self, id: u64) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodInfo {
    pub period: u8,
    pub start_t: f64,
    pub end_t: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    pub source: String,
    pub competition: String,
    pub season: String,
    pub date: String,
    pub teams: [Team; 2],
    pub events: Vec<Event>,
    pub periods: Vec<PeriodInfo>,
    pub fallbacks: Vec<String>,
}

impl Match {
    pub fn team(&self, s: Side) -> &Team {
        &self.teams[s.idx()]
    }
    pub fn duration(&self) -> f64 {
        self.periods.iter().filter(|p| p.period <= 4).map(|p| p.end_t).fold(0.0, f64::max)
    }
    pub fn player_name(&self, side: Side, id: Option<u64>) -> String {
        id.and_then(|i| self.team(side).player(i)).map(|p| p.short.clone()).unwrap_or_else(|| "Unknown".into())
    }
    pub fn find_player(&self, id: u64) -> Option<(Side, &Player)> {
        for s in Side::both() {
            if let Some(p) = self.team(s).player(id) {
                return Some((s, p));
            }
        }
        None
    }
    /// Expanded minute (0-based float) for an event time.
    pub fn minute_of(&self, t: f64) -> f64 {
        t / 60.0
    }
    pub fn shots(&self, s: Side) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |e| e.side == s && e.kind == Kind::Shot && !e.shot.as_ref().unwrap().own_goal)
    }
    pub fn goals_timeline(&self) -> Vec<(Side, &Event)> {
        let mut v = vec![];
        for e in &self.events {
            if let Some(sh) = &e.shot {
                if sh.outcome == ShotOutcome::Goal && e.period <= 4 {
                    let side = if sh.own_goal { e.side.other() } else { e.side };
                    v.push((side, e));
                }
            }
        }
        v
    }
}

pub fn short_name(full: &str) -> String {
    let parts: Vec<&str> = full.split_whitespace().collect();
    if parts.len() <= 1 {
        return full.to_string();
    }
    let particles = ["de", "da", "van", "von", "der", "di", "del", "dos", "la", "le", "ter", "ten"];
    let mut i = parts.len() - 1;
    while i > 0 && particles.contains(&parts[i - 1].to_lowercase().as_str()) {
        i -= 1;
    }
    let s = parts[i..].join(" ");
    if s.len() <= 2 { full.to_string() } else { s }
}
