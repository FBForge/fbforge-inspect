//! Native update preferences and scheduling decisions. These are not package,
//! process or execution evidence. The executor must re-evaluate current policy
//! and all installation preconditions immediately before changing an installation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePreferences {
    pub search: bool,
    pub download: bool,
    pub install: bool,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            search: true,
            download: false,
            install: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateOverrides {
    pub search: Option<bool>,
    pub download: Option<bool>,
    pub install: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct AdministrativeUpdatePolicy {
    pub updates_allowed: bool,
    pub automatic: UpdateOverrides,
}

impl Default for AdministrativeUpdatePolicy {
    fn default() -> Self {
        Self {
            updates_allowed: true,
            automatic: UpdateOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutomaticInstallConsent {
    NotGranted,
    Granted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveSetting {
    pub enabled: bool,
    pub is_managed: bool,
}

pub struct EffectiveUpdatePolicy {
    search: EffectiveSetting,
    download: EffectiveSetting,
    install: EffectiveSetting,
    updates_allowed: bool,
    consent: AutomaticInstallConsent,
}

/// Admin values override tool overrides, which override global user settings.
/// A managed request to automate installation still cannot grant user consent.
pub fn resolve_update_policy(
    global: UpdatePreferences,
    tool: UpdateOverrides,
    administration: AdministrativeUpdatePolicy,
    consent: AutomaticInstallConsent,
) -> EffectiveUpdatePolicy {
    fn resolve(global: bool, tool: Option<bool>, managed: Option<bool>) -> EffectiveSetting {
        EffectiveSetting {
            enabled: managed.or(tool).unwrap_or(global),
            is_managed: managed.is_some(),
        }
    }
    EffectiveUpdatePolicy {
        search: resolve(global.search, tool.search, administration.automatic.search),
        download: resolve(
            global.download,
            tool.download,
            administration.automatic.download,
        ),
        install: resolve(
            global.install,
            tool.install,
            administration.automatic.install,
        ),
        updates_allowed: administration.updates_allowed,
        consent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStage {
    Search,
    Download,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTrigger {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UpdateTiming {
    pub paused_until: Option<u64>,
    /// Applies to the concrete release selected by the caller, not other tools.
    pub deferred_until: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDecision {
    Ready,
    Disabled,
    BlockedByAdministration,
    ConsentRequired,
    PausedUntil(u64),
    DeferredUntil(u64),
}

impl EffectiveUpdatePolicy {
    pub fn search(&self) -> EffectiveSetting {
        self.search
    }
    pub fn download(&self) -> EffectiveSetting {
        self.download
    }
    pub fn install(&self) -> EffectiveSetting {
        self.install
    }

    pub fn decide(
        &self,
        stage: UpdateStage,
        trigger: UpdateTrigger,
        timing: UpdateTiming,
        now: u64,
    ) -> UpdateDecision {
        if !self.updates_allowed {
            return UpdateDecision::BlockedByAdministration;
        }
        // An explicit single operation does not opt the user into future updates.
        if trigger == UpdateTrigger::Manual {
            return UpdateDecision::Ready;
        }
        let setting = match stage {
            UpdateStage::Search => self.search,
            UpdateStage::Download => self.download,
            UpdateStage::Install => self.install,
        };
        if !setting.enabled {
            return UpdateDecision::Disabled;
        }
        if let Some(until) = timing.paused_until.filter(|until| *until > now) {
            return UpdateDecision::PausedUntil(until);
        }
        if stage != UpdateStage::Search {
            if let Some(until) = timing.deferred_until.filter(|until| *until > now) {
                return UpdateDecision::DeferredUntil(until);
            }
        }
        if stage == UpdateStage::Install && self.consent != AutomaticInstallConsent::Granted {
            return UpdateDecision::ConsentRequired;
        }
        UpdateDecision::Ready
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchInterval(u64);

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidSearchInterval;
impl std::fmt::Display for InvalidSearchInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Das Suchintervall muss zwischen 15 Minuten und sieben Tagen liegen.")
    }
}
impl std::error::Error for InvalidSearchInterval {}

#[derive(Debug, PartialEq, Eq)]
pub enum SearchDue {
    Now,
    At(u64),
    ClockRollback,
    ClockOverflow,
}

impl Default for SearchInterval {
    fn default() -> Self {
        Self(6 * 60 * 60)
    }
}
impl SearchInterval {
    pub fn new(seconds: u64) -> Result<Self, InvalidSearchInterval> {
        if !(900..=604_800).contains(&seconds) {
            return Err(InvalidSearchInterval);
        }
        Ok(Self(seconds))
    }

    /// Record the last attempt, including failures. Retry classification and
    /// backoff belong to the task runner, so failures cannot cause a tight loop.
    pub fn next_search(self, last_attempt_at: Option<u64>, now: u64) -> SearchDue {
        let Some(previous) = last_attempt_at else {
            return SearchDue::Now;
        };
        if now < previous {
            return SearchDue::ClockRollback;
        }
        let Some(next) = previous.checked_add(self.0) else {
            return SearchDue::ClockOverflow;
        };
        if now >= next {
            SearchDue::Now
        } else {
            SearchDue::At(next)
        }
    }
}
