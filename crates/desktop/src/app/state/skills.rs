use std::sync::mpsc;

use super::super::{ChaiApp, Screen, STATUS_INTERVAL_FRAMES};

impl ChaiApp {
    /// Poll for skills fetch result and optionally start a new fetch. Call each frame.
    ///
    /// Skills are fetched on-demand: immediately when the cache is empty (e.g.
    /// after profile switch, gateway stop, or profile override change), and
    /// periodically only when the Skills or Agent screen is active. Other screens do not
    /// trigger periodic skills fetches, since skills data rarely changes while
    /// the gateway is running.
    pub(crate) fn poll_skills_fetch(&mut self) {
        if let Some(rx) = &self.skills_fetch_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Ok(skills) = result {
                    self.cached_skills = Some(skills);
                }
                self.skills_fetch_receiver = None;
            }
        }
        if self.skills_fetch_receiver.is_some() {
            return;
        }

        let need_immediate = self.cached_skills.is_none();
        let screen_active = matches!(self.current_screen, Screen::Skills | Screen::Agent);

        // Always fetch immediately when the cache is empty (e.g. after
        // invalidation from profile switch / gateway stop). Otherwise only
        // refresh periodically when the Skills or Agent screen is active.
        if !need_immediate && !screen_active {
            return;
        }

        self.frames_since_skills_fetch = self.frames_since_skills_fetch.saturating_add(1);
        if need_immediate || self.frames_since_skills_fetch >= STATUS_INTERVAL_FRAMES {
            self.frames_since_skills_fetch = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = self.cached_profile_override.clone();
            std::thread::spawn(move || {
                let result = fetch_skills(profile_override.as_deref());
                let _ = tx.send(result);
            });
            self.skills_fetch_receiver = Some(rx);
        }
    }

    /// Invalidate the skills cache, forcing a refresh on the next poll.
    pub(crate) fn invalidate_skills_cache(&mut self) {
        self.cached_skills = None;
        // Reset frame counter so the immediate fetch on next poll starts a
        // fresh interval afterward.
        self.frames_since_skills_fetch = 0;
    }
}

/// Load skills from the default skills directory. Runs in a background thread.
fn fetch_skills(profile_override: Option<&str>) -> Result<Vec<lib::skills::SkillEntry>, String> {
    let (_, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
    let chai_home = &paths.chai_home;
    let skills_root = lib::config::default_skills_dir(chai_home);
    lib::skills::load_skills(skills_root.as_path()).map_err(|e| e.to_string())
}
