//! League data: the cached catalog and scouting index, their background refresh, and the UI state of the
//! league browser and the league scout. The files come from the nightly ingest (`matchhub ingest`) over
//! plain HTTPS; nothing here scrapes.

use crate::App;
use matchhub::catalog::*;
use std::sync::{Arc, Mutex};

/// A refresh is attempted at start when the cached files are older than this.
const STALE_SECS: u64 = 6 * 3600;

#[derive(Default)]
pub struct DataJob {
    pub running: bool,
    pub error: Option<String>,
    pub result: Option<(Catalog, Option<ScoutIndex>)>,
    /// seconds since start of the last successful refresh, for the "updated" caption
    pub done_at: Option<std::time::Instant>,
}

pub struct DataState {
    pub store: CatalogStore,
    pub catalog: Option<Arc<Catalog>>,
    pub index: Option<Arc<ScoutIndex>>,
    pub job: Arc<Mutex<DataJob>>,
    pub refresh_started: bool,
    /// Matches filter: 0 Saved, 1 All 7, then the seven leagues in `League::ALL` order
    pub league: usize,
    pub query: String,
    /// Players screen mode: false = Match squad, true = League Scout
    pub scout_on: bool,
    /// Scout filters: league 0 All 7, 1 Top 5, then the seven; role 0 All then `ScoutRole::ALL`; minutes index into MINUTES
    pub s_league: usize,
    pub s_role: usize,
    pub s_min: usize,
    pub s_query: String,
    /// sort column of the leaderboard (index into COLUMNS) and direction
    pub s_sort: usize,
    pub s_desc: bool,
    pub s_rows: usize,
    /// Compare: search text for league players
    pub c_query: String,
    scenes: std::collections::HashMap<u64, Arc<matchhub::render::svg::Scene>>,
}

pub const MINUTES: [f32; 3] = [270.0, 450.0, 900.0];
/// Leaderboard columns: header, metric label in the index.
pub const COLUMNS: [(&str, &str); 4] = [("xG/90", "xG"), ("xA/90", "Inferred xA"), ("Prog/90", "Progressive actions"), ("VAEP/90", "VAEP")];

impl DataState {
    pub fn open(root: &std::path::Path) -> DataState {
        let store = CatalogStore::new(&root.join("catalog"));
        let catalog = store.load_catalog().map(Arc::new);
        let index = store.load_scout().map(Arc::new);
        crate::library::set_scout(index.clone());
        DataState { store, catalog, index, job: Default::default(), refresh_started: false, league: 0, query: String::new(), scout_on: false, s_league: 0, s_role: 0, s_min: 0, s_query: String::new(), s_sort: 0, s_desc: true, s_rows: 40, c_query: String::new(), scenes: Default::default() }
    }
    /// The season card of a player as a drawable scene, built once per player.
    pub fn scout_scene(&mut self, ix: &ScoutIndex, row: &ScoutPlayerRow, accent: &str) -> Arc<matchhub::render::svg::Scene> {
        self.scenes.entry(row.id).or_insert_with(|| Arc::new(matchhub::render::scout::scout_scene(row, ix, accent))).clone()
    }
    pub fn league_filter(&self) -> Option<LeagueFilter> {
        match self.league {
            0 => None,
            1 => Some(LeagueFilter::All),
            n => League::ALL.get(n - 2).map(|l| LeagueFilter::One(*l)),
        }
    }
    pub fn scout_filter(&self) -> LeagueFilter {
        match self.s_league {
            0 => LeagueFilter::All,
            1 => LeagueFilter::Top5,
            n => League::ALL.get(n - 2).map(|l| LeagueFilter::One(*l)).unwrap_or(LeagueFilter::All),
        }
    }
    pub fn scout_role(&self) -> Option<ScoutRole> {
        if self.s_role == 0 { None } else { ScoutRole::ALL.get(self.s_role - 1).copied() }
    }
    /// Leaderboard rows for the current filters, sorted by the chosen column.
    pub fn leaderboard(&self) -> Vec<&ScoutPlayerRow> {
        let Some(ix) = &self.index else { return vec![] };
        let q = self.s_query.trim().to_lowercase();
        let mut v: Vec<&ScoutPlayerRow> = ix.pool(self.scout_role(), self.scout_filter(), MINUTES[self.s_min.min(2)]).into_iter().filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q) || p.team.to_lowercase().contains(&q)).collect();
        let label = COLUMNS[self.s_sort.min(COLUMNS.len() - 1)].1;
        v.sort_by(|a, b| {
            let o = a.value(label).partial_cmp(&b.value(label)).unwrap_or(std::cmp::Ordering::Equal);
            (if self.s_desc { o.reverse() } else { o }).then(a.id.cmp(&b.id))
        });
        v
    }
}

impl App {
    /// Where the published data branch is read from: the same repository and token as the scrape relay.
    pub fn data_remote(&self) -> Option<Remote> {
        let repo = self.prefs.relay_repo.trim();
        if repo.is_empty() || !repo.contains('/') {
            return None;
        }
        Some(Remote { repo: repo.to_string(), branch: "data-catalog".into(), token: Some(self.prefs.relay_token.clone()).filter(|t| !t.trim().is_empty()) })
    }

    /// Takes a finished refresh into the app and starts one when the cache is stale. Called every frame.
    pub fn poll_data(&mut self, ctx: &egui::Context) {
        let finished = self.data.job.lock().unwrap().result.take();
        if let Some((c, s)) = finished {
            self.data.catalog = Some(Arc::new(c));
            self.data.scenes.clear();
            if let Some(s) = s {
                self.data.index = Some(Arc::new(s));
            }
            crate::library::set_scout(self.data.index.clone());
        }
        if !self.data.refresh_started {
            self.data.refresh_started = true;
            let stale = self.data.store.age_secs().map(|a| a > STALE_SECS).unwrap_or(true);
            if stale {
                self.refresh_data(ctx);
            }
        }
    }

    /// Downloads catalog.json and scout_index.json in the background.
    pub fn refresh_data(&mut self, ctx: &egui::Context) {
        let Some(remote) = self.data_remote() else {
            self.data.job.lock().unwrap().error = Some("Set the repository under Settings > Scrape relay to load league data".into());
            return;
        };
        {
            let mut j = self.data.job.lock().unwrap();
            if j.running {
                return;
            }
            j.running = true;
            j.error = None;
        }
        let job = self.data.job.clone();
        let store = self.data.store.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = store.refresh(&remote);
            let mut j = job.lock().unwrap();
            j.running = false;
            match r {
                Ok(res) => {
                    j.result = Some(res);
                    j.done_at = Some(std::time::Instant::now());
                }
                Err(e) => j.error = Some(format!("{e:#}")),
            }
            ctx.request_repaint();
        });
    }
}
