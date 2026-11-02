//! Which pets are on screen (spec §5.7): neediest first, at most one Gloomy, capped at `max_visible`.
//! The one-Gloomy rule gives way when keeping it would leave fewer than `min_visible` pets.
use std::collections::HashMap;

use uuid::Uuid;

use crate::clock::Time;
use crate::mood::Mood;
use crate::pet::{Pet, StateKind};

#[derive(Debug, PartialEq)]
pub struct Selection {
    pub visible: Vec<Uuid>,
    /// Hungry or Gloomy Active pets left off screen: the menu-bar badge count.
    pub hidden_needy: usize,
}

/// `moods` must hold every Active pet. `last_shown` breaks ties: the pet seen longest ago (or never) wins.
pub fn choose(pets: &[Pet], moods: &HashMap<Uuid, Mood>, last_shown: &HashMap<Uuid, Time>, min_visible: usize, max_visible: usize) -> Selection {
    let mut active: Vec<(&Pet, Mood)> = pets
        .iter()
        .filter(|p| p.state.kind == StateKind::Active)
        .map(|p| (p, moods[&p.id]))
        .collect();
    active.sort_by(|(a, ma), (b, mb)| mb.cmp(ma).then(last_shown.get(&a.id).cmp(&last_shown.get(&b.id))).then(a.created_at.cmp(&b.created_at)));
    let mut visible = Vec::new();
    let mut gloomy_shown = false;
    for (p, m) in &active {
        if visible.len() == max_visible {
            break;
        }
        if *m == Mood::Gloomy {
            if gloomy_shown {
                continue;
            }
            gloomy_shown = true;
        }
        visible.push(p.id);
    }
    // the one-Gloomy rule never leaves the screen below the minimum
    for (p, _) in &active {
        if visible.len() >= min_visible.min(max_visible) {
            break;
        }
        if !visible.contains(&p.id) {
            visible.push(p.id);
        }
    }
    let hidden_needy = active.iter().filter(|(p, m)| *m >= Mood::Hungry && !visible.contains(&p.id)).count();
    Selection { visible, hidden_needy }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mood::Pace;
    use crate::pet::{Activity, ActivitySource, Project, Species};
    use chrono::Duration;

    fn pets(n: usize) -> Vec<Pet> {
        let t0: Time = "2026-10-01T00:00:00Z".parse().unwrap();
        (0..n)
            .map(|i| {
                let at = t0 + Duration::minutes(i as i64);
                let mut p = Pet::new(at, "p", Species::Cat, Project::default(), Activity { last: at, source: ActivitySource::LocalCommit }, Pace::Normal);
                p.adopt(at).unwrap();
                p
            })
            .collect()
    }

    #[test]
    fn neediest_first_one_gloomy_and_ties_by_last_shown() {
        let ps = pets(5);
        let ids: Vec<Uuid> = ps.iter().map(|p| p.id).collect();
        let moods: HashMap<_, _> = [Mood::Gloomy, Mood::Gloomy, Mood::Content, Mood::Hungry, Mood::Content].into_iter().enumerate().map(|(i, m)| (ids[i], m)).collect();
        let shown: HashMap<_, _> = [(ids[2], "2026-10-10T00:00:00Z".parse().unwrap())].into();
        let s = choose(&ps, &moods, &shown, 1, 3);
        // one gloomy (the older one on ties), the hungry one, then the content pet never shown (4) over 2
        assert_eq!(s.visible, vec![ids[0], ids[3], ids[4]]);
        assert_eq!(s.hidden_needy, 1);
    }

    #[test]
    fn only_active_pets_and_cap() {
        let mut ps = pets(4);
        let at = ps[0].created_at;
        ps[0].snooze(at, None).unwrap();
        let moods: HashMap<_, _> = ps.iter().map(|p| (p.id, Mood::Joyful)).collect();
        let s = choose(&ps, &moods, &HashMap::new(), 1, 2);
        assert_eq!(s.visible, vec![ps[1].id, ps[2].id]);
        assert_eq!(choose(&ps, &moods, &HashMap::new(), 1, 8).visible.len(), 3);
    }

    #[test]
    fn a_second_gloomy_pet_shows_when_the_minimum_needs_it() {
        let ps = pets(3);
        let moods: HashMap<_, _> = ps.iter().map(|p| (p.id, Mood::Gloomy)).collect();
        assert_eq!(choose(&ps, &moods, &HashMap::new(), 1, 3).visible, vec![ps[0].id]);
        assert_eq!(choose(&ps, &moods, &HashMap::new(), 2, 3).visible, vec![ps[0].id, ps[1].id]);
        assert_eq!(choose(&ps, &moods, &HashMap::new(), 0, 3).visible, vec![ps[0].id]);
    }
}
