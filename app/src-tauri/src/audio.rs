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

/// Whether there's a sound output to play on, for the Test button: playing happens later, on its own thread.
pub fn check_output() -> Result<(), String> {
    rodio::DeviceSinkBuilder::open_default_sink().map(drop).map_err(|e| format!("No sound output device: {e}"))
}

/// Plays sounds one after another on a background thread.
#[derive(Clone)]
pub struct Player(mpsc::Sender<(Sound, f32)>);

impl Player {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<(Sound, f32)>();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let device = match rodio::DeviceSinkBuilder::open_default_sink() {
                    Ok(d) => d,
                    Err(e) => {
                        crate::log::warn(&format!("no sound output device: {e}"));
                        continue;
                    }
                };
                // play this and anything queued meanwhile, then close the device again
                let mut next = Some(first);
                while let Some((sound, volume)) = next {
                    match rodio::play(device.mixer(), Cursor::new(bytes(sound))) {
                        Ok(player) => {
                            player.set_volume(volume);
                            player.sleep_until_end();
                        }
                        Err(e) => crate::log::warn(&format!("could not play {sound:?}: {e}")),
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

/// Whether the sad loop may play: it's on, sound is allowed, and something on screen is gloomy.
pub fn gloom_allowed(s: &Settings, any_gloomy: bool, presentation: bool, full_screen: bool, local_minute: u32) -> bool {
    s.sad_music && any_gloomy && allowed(s, None, presentation, full_screen, local_minute)
}

/// "Solar winds (Perfect loop, smaller size)" by cookies+policy, CC0 (assets/audio/CREDITS.md).
const SAD_LOOP: &[u8] = include_bytes!("../../../assets/audio/sad-loop.mp3");
/// The loop sits well under the pet sounds.
const SAD_LEVEL: f32 = 0.35;

/// The one sad loop: fades in over 3 s when asked to play, out over 2 s when asked to stop, and keeps the
/// output device open only while it's audible.
#[derive(Clone)]
pub struct Gloom(mpsc::Sender<Option<f32>>);

impl Gloom {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<Option<f32>>();
        std::thread::spawn(move || {
            let mut target: Option<f32> = None;
            let mut level = 0.0f32;
            let mut playing: Option<(rodio::MixerDeviceSink, rodio::Player)> = None;
            loop {
                // idle: block until asked to play; playing: tick every 50 ms to fade
                let msg = if playing.is_none() { rx.recv().map(Some).map_err(|_| ()) } else { Ok(rx.recv_timeout(std::time::Duration::from_millis(50)).ok()) };
                match msg {
                    Err(()) => return,
                    Ok(Some(t)) => target = t,
                    Ok(None) => {}
                }
                if playing.is_none() && target.is_some() {
                    let opened = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| format!("no sound output device: {e}")).and_then(|device| {
                        let looped = rodio::Decoder::new_looped(Cursor::new(SAD_LOOP)).map_err(|e| format!("could not play the sad loop: {e}"))?;
                        let player = rodio::Player::connect_new(device.mixer());
                        player.set_volume(0.0);
                        player.append(looped);
                        Ok((device, player))
                    });
                    match opened {
                        Ok(p) => {
                            level = 0.0;
                            playing = Some(p);
                        }
                        Err(e) => {
                            crate::log::warn(&e);
                            target = None;
                            continue;
                        }
                    }
                }
                if let Some((_, player)) = &playing {
                    let want = target.unwrap_or(0.0) * SAD_LEVEL;
                    let step = if want > level { 0.05 / 3.0 } else { 0.05 / 2.0 } * SAD_LEVEL.max(want);
                    level = if want > level { (level + step).min(want) } else { (level - step).max(want) };
                    player.set_volume(level);
                    if target.is_none() && level <= 0.0 {
                        playing = None;
                    }
                }
            }
        });
        Gloom(tx)
    }

    /// `Some(volume 0–100)` plays (or changes the volume), `None` fades out and stops.
    pub fn set(&self, volume: Option<u32>) {
        let _ = self.0.send(volume.map(|v| v.min(100) as f32 / 100.0));
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
    fn sad_loop_is_off_by_default_and_needs_a_gloomy_pet() {
        let mut s = Settings::default();
        assert!(!gloom_allowed(&s, true, false, false, 12 * 60), "off by default");
        s.sad_music = true;
        assert!(!gloom_allowed(&s, true, false, false, 12 * 60), "sound itself is off");
        s.sound = true;
        assert!(gloom_allowed(&s, true, false, false, 12 * 60));
        assert!(!gloom_allowed(&s, false, false, false, 12 * 60), "nothing gloomy on screen");
        assert!(!gloom_allowed(&s, true, false, true, 12 * 60), "full screen");
        assert!(!gloom_allowed(&s, true, true, false, 12 * 60), "presentation");
        assert!(!gloom_allowed(&s, true, false, false, 23 * 60), "quiet hours");
    }

    #[test]
    fn every_sound_is_bundled_and_decodes() {
        let all = [Species::Cat, Species::Dog, Species::Hamster, Species::Parrot]
            .into_iter()
            .flat_map(|sp| [Sound::Greeting(sp), Sound::Happy(sp), Sound::Sad(sp)])
            .chain([Sound::Celebrate, Sound::Wake, Sound::Tick]);
        let looped = rodio::Decoder::try_from(Cursor::new(SAD_LOOP)).expect("the sad loop decodes");
        assert!(rodio::Source::total_duration(&looped).is_none_or(|t| t.as_secs_f32() < 60.0), "the sad loop is a short loop");
        for sound in all {
            let d = rodio::Decoder::try_from(Cursor::new(bytes(sound))).unwrap_or_else(|e| panic!("{sound:?}: {e}"));
            assert!(rodio::Source::total_duration(&d).is_none_or(|t| t.as_secs_f32() < 4.0), "{sound:?} is short");
        }
    }
}
