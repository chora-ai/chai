use std::sync::mpsc;

use super::super::{ChaiApp, STATUS_INTERVAL_FRAMES};

impl ChaiApp {
    /// Poll for skills fetch result and optionally start a new fetch. Call each frame.
    /// Skills are refreshed on the same cadence as gateway status (~0.5 Hz), or immediately
    /// when the cache is empty.
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
        self.frames_since_skills_fetch = self.frames_since_skills_fetch.saturating_add(1);
        if need_immediate || self.frames_since_skills_fetch >= STATUS_INTERVAL_FRAMES {
            self.frames_since_skills_fetch = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = fetch_skills();
                let _ = tx.send(result);
            });
            self.skills_fetch_receiver = Some(rx);
        }
    }

    /// Invalidate the skills cache, forcing a refresh on the next poll.
    pub(crate) fn invalidate_skills_cache(&mut self) {
        self.cached_skills = None;
    }
}

/// Load skills from the default skills directory. Runs in a background thread.
fn fetch_skills() -> Result<Vec<lib::skills::SkillEntry>, String> {
    let (_, paths) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let chai_home = &paths.chai_home;
    let skills_root = lib::config::default_skills_dir(chai_home);
    lib::skills::load_skills(skills_root.as_path()).map_err(|e| e.to_string())
}
