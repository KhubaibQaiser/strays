//! Sound (§8.4, §10.2): native playback with rodio, never in the web view. Off by default.
//! The output device is opened only while something plays, so silence costs nothing.

use std::io::Cursor;
use std::sync::mpsc;

use strays_core::pet::Species;
use strays_core::settings::Settings;

use crate::state::Cue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Greeting(Species),
    Happy(Species),
    Sad(Species),
    Celebrate,
    Wake,
    Tick,
}

/// The sound a cue makes, if any.
pub fn for_cue(cue: Cue, species: Species) -> Option<Sound> {
    match cue {
        Cue::Eat => Some(Sound::Happy(species)),
        Cue::Celebrate { .. } | Cue::Graduate { .. } => Some(Sound::Celebrate),
        Cue::Wake => Some(Sound::Wake),
        Cue::Postcard | Cue::Hello => Some(Sound::Greeting(species)),
        Cue::FallAsleep => None,
    }
}

/// Whether a sound may play now: sound on, volume up, species not muted, and not during quiet hours,
/// Presentation mode or a full-screen app (the same places nudges stay quiet).
pub fn allowed(s: &Settings, species: Option<Species>, presentation: bool, full_screen: bool, local_minute: u32) -> bool {
    s.sound && s.volume > 0 && !presentation && !full_screen && !s.is_quiet(local_minute) && species.is_none_or(|sp| !s.muted_species.contains(&sp))
}

fn bytes(sound: Sound) -> &'static [u8] {
    macro_rules! clip {
        ($name:literal) => {
            include_bytes!(concat!("../../../assets/audio/", $name, ".mp3")).as_slice()
        };
    }
    use Species::*;
    match sound {
        Sound::Greeting(Cat) => clip!("cat-greeting"),
        Sound::Happy(Cat) => clip!("cat-happy"),
        Sound::Sad(Cat) => clip!("cat-sad"),
        Sound::Greeting(Dog) => clip!("dog-greeting"),
        Sound::Happy(Dog) => clip!("dog-happy"),
        Sound::Sad(Dog) => clip!("dog-sad"),
        Sound::Greeting(Hamster) => clip!("hamster-greeting"),
        Sound::Happy(Hamster) => clip!("hamster-happy"),
        Sound::Sad(Hamster) => clip!("hamster-sad"),
        Sound::Greeting(Parrot) => clip!("parrot-greeting"),
        Sound::Happy(Parrot) => clip!("parrot-happy"),
        Sound::Sad(Parrot) => clip!("parrot-sad"),
        Sound::Celebrate => clip!("celebrate"),
        Sound::Wake => clip!("wake"),
        Sound::Tick => clip!("tick"),
    }
}

/// Plays sounds one after another on a background thread.
#[derive(Clone)]
pub struct Player(mpsc::Sender<(Sound, f32)>);

impl Player {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<(Sound, f32)>();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let Ok(device) = rodio::DeviceSinkBuilder::open_default_sink() else {
                    eprintln!("no audio output device");
                    continue;
                };
                // play this and anything queued meanwhile, then close the device again
                let mut next = Some(first);
                while let Some((sound, volume)) = next {
                    match rodio::play(device.mixer(), Cursor::new(bytes(sound))) {
                        Ok(player) => {
                            player.set_volume(volume);
                            player.sleep_until_end();
                        }
                        Err(e) => eprintln!("could not play {sound:?}: {e}"),
                    }
                    next = rx.try_recv().ok();
                }
            }
        });
        Player(tx)
    }

    /// `volume` is the 0–100 setting.
    pub fn play(&self, sound: Sound, volume: u32) {
        let _ = self.0.send((sound, volume.min(100) as f32 / 100.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_is_off_by_default_and_respects_quiet_places() {
        let mut s = Settings::default();
        assert!(!allowed(&s, None, false, false, 12 * 60), "off by default (§10.2)");
        s.sound = true;
        assert!(allowed(&s, Some(Species::Cat), false, false, 12 * 60));
        assert!(!allowed(&s, None, true, false, 12 * 60), "presentation");
        assert!(!allowed(&s, None, false, true, 12 * 60), "full screen");
        assert!(!allowed(&s, None, false, false, 23 * 60), "quiet hours");
        s.muted_species = vec![Species::Cat];
        assert!(!allowed(&s, Some(Species::Cat), false, false, 12 * 60));
        assert!(allowed(&s, Some(Species::Dog), false, false, 12 * 60));
        s.volume = 0;
        assert!(!allowed(&s, Some(Species::Dog), false, false, 12 * 60));
    }

    #[test]
    fn every_sound_is_bundled_and_decodes() {
        let all = [Species::Cat, Species::Dog, Species::Hamster, Species::Parrot]
            .into_iter()
            .flat_map(|sp| [Sound::Greeting(sp), Sound::Happy(sp), Sound::Sad(sp)])
            .chain([Sound::Celebrate, Sound::Wake, Sound::Tick]);
        for sound in all {
            let d = rodio::Decoder::try_from(Cursor::new(bytes(sound))).unwrap_or_else(|e| panic!("{sound:?}: {e}"));
            assert!(rodio::Source::total_duration(&d).is_none_or(|t| t.as_secs_f32() < 4.0), "{sound:?} is short");
        }
    }
}
