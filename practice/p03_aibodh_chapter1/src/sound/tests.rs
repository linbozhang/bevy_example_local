use bevy::audio::{Decodable, PlaybackMode, Source};

use super::*;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Assets::<Clip>::default());
    // Exercise the chapter systems without opening a device or window.
    install_systems(&mut app);
    app.update();
    app
}

fn music_player(app: &mut App) -> Option<(Entity, MusicTrack, PlaybackSettings)> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &ChapterSound, &PlaybackSettings)>();
    let mut music = query
        .iter(world)
        .filter_map(|(entity, sound, settings)| match sound {
            ChapterSound::Music(track) => Some((entity, *track, *settings)),
            _ => None,
        });
    let first = music.next();
    assert!(music.next().is_none(), "only one music player may exist");
    first
}

#[test]
fn music_switches_tracks_without_restarting_on_pause_or_mute() {
    let mut app = headless_app();
    assert!(music_player(&mut app).is_none());

    app.world_mut().resource_mut::<SoundSettings>().music = MusicTrack::Menu;
    app.update();
    let (menu, track, settings) = music_player(&mut app).unwrap();
    assert_eq!(track, MusicTrack::Menu);
    assert!(matches!(settings.mode, PlaybackMode::Loop));

    {
        let mut settings = app.world_mut().resource_mut::<SoundSettings>();
        settings.music = MusicTrack::Gameplay;
        settings.paused = true;
        settings.muted = true;
    }
    app.update();
    let (gameplay, track, settings) = music_player(&mut app).unwrap();
    assert_ne!(menu, gameplay);
    assert_eq!(track, MusicTrack::Gameplay);
    assert!(settings.paused && settings.muted);

    {
        let mut settings = app.world_mut().resource_mut::<SoundSettings>();
        settings.paused = false;
        settings.muted = false;
    }
    app.update();
    let (same, _, settings) = music_player(&mut app).unwrap();
    assert_eq!(same, gameplay);
    assert!(!settings.paused && !settings.muted);

    app.world_mut().resource_mut::<SoundSettings>().music = MusicTrack::Silent;
    app.update();
    assert!(music_player(&mut app).is_none());
}

fn effects(app: &mut App) -> Vec<SoundCue> {
    let world = app.world_mut();
    world
        .query::<(&ChapterSound, &PlaybackSettings)>()
        .iter(world)
        .filter_map(|(sound, playback)| match sound {
            ChapterSound::Effect { cue, .. } => {
                assert!(matches!(playback.mode, PlaybackMode::Despawn));
                Some(*cue)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn suppressed_cues_are_drained_and_unplayed_effects_expire() {
    let mut app = headless_app();
    app.world_mut().resource_mut::<SoundSettings>().paused = true;
    app.world_mut().write_message(SoundCue::Pickup);
    app.world_mut().write_message(SoundCue::MenuClick);
    app.update();
    assert_eq!(effects(&mut app), [SoundCue::MenuClick]);

    app.world_mut().resource_mut::<SoundSettings>().muted = true;
    app.world_mut().write_message(SoundCue::MenuClick);
    app.update();
    assert_eq!(effects(&mut app), [SoundCue::MenuClick]);

    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        QUEUE_TIMEOUT,
    ));
    *app.world_mut().resource_mut::<SoundSettings>() = SoundSettings::default();
    app.update();
    assert!(effects(&mut app).is_empty());

    app.world_mut().write_message(SoundCue::Hit);
    app.update();
    assert_eq!(effects(&mut app), [SoundCue::Hit]);
}

#[test]
fn generated_clips_decode_to_finite_nonclipping_samples() {
    for clip in SoundCue::ALL.map(synth::cue).into_iter().chain([
        synth::music(MusicTrack::Menu),
        synth::music(MusicTrack::Gameplay),
    ]) {
        let decoder = clip.decoder();
        let rate = f64::from(decoder.sample_rate().get());
        let duration = decoder.total_duration().unwrap();
        let samples: Vec<_> = decoder.collect();
        assert_eq!(
            samples.len(),
            (duration.as_secs_f64() * rate).round() as usize
        );
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples.last(), Some(&0.0));
        assert!(samples.iter().any(|sample| sample.abs() > 0.1));
        assert!(samples.iter().all(|sample| sample.abs() < 1.0));
    }
}
