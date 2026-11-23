//! Chapter 11 audio for both views, with no dependency on gameplay modules.
//! The synthesized clips are original; see `LICENSE-MIT` in the crate root.

mod synth;

use std::time::Duration;

use bevy::{
    audio::{AddAudioSource, Volume},
    prelude::*,
    transform::TransformSystems,
};

use synth::Clip;

/// Add after `DefaultPlugins`. Works the same in 2D and 3D.
pub(crate) struct SoundPlugin;

/// Write a cue when the action happens, not when its input is pressed.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SoundCue {
    Pickup,
    PlayerCast,
    EnemyCast,
    Hit,
    PlayerDeath,
    EnemyDeath,
    MenuClick,
    Jump,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MusicTrack {
    #[default]
    Silent,
    Menu,
    Gameplay,
}

/// Set from game-state or UI systems. Select the same track when resuming to
/// continue from where it paused rather than restart it.
#[derive(Resource, Default)]
pub(crate) struct SoundSettings {
    pub(crate) music: MusicTrack,
    /// Freeze music and in-flight gameplay cues. Menu clicks can still play.
    pub(crate) paused: bool,
    /// Mute existing playback and discard new cues while muted.
    pub(crate) muted: bool,
}

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Clip>();
        install_systems(app);
    }
}

fn install_systems(app: &mut App) {
    app.init_resource::<SoundSettings>()
        .add_message::<SoundCue>()
        .add_systems(Startup, generate_clips)
        // Gameplay sends cues in Update. Bevy starts queued audio after transform
        // propagation, so changes to pending settings apply before it starts.
        .add_systems(
            PostUpdate,
            (update_music, maintain_players, play_cues)
                .chain()
                .before(TransformSystems::Propagate),
        );
}

#[derive(Resource)]
struct SoundClips {
    menu: Handle<Clip>,
    gameplay: Handle<Clip>,
    cues: [Handle<Clip>; SoundCue::ALL.len()],
}

impl SoundCue {
    const ALL: [Self; 8] = [
        Self::Pickup,
        Self::PlayerCast,
        Self::EnemyCast,
        Self::Hit,
        Self::PlayerDeath,
        Self::EnemyDeath,
        Self::MenuClick,
        Self::Jump,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

fn generate_clips(mut commands: Commands, mut assets: ResMut<Assets<Clip>>) {
    commands.insert_resource(SoundClips {
        menu: assets.add(synth::music(MusicTrack::Menu)),
        gameplay: assets.add(synth::music(MusicTrack::Gameplay)),
        cues: SoundCue::ALL.map(|cue| assets.add(synth::cue(cue))),
    });
}

#[derive(Component)]
enum ChapterSound {
    Music(MusicTrack),
    Effect { cue: SoundCue, queued_at: Duration },
}

impl ChapterSound {
    fn pauses(&self) -> bool {
        !matches!(
            self,
            Self::Effect {
                cue: SoundCue::MenuClick,
                ..
            }
        )
    }
}

fn update_music(
    mut commands: Commands,
    settings: Res<SoundSettings>,
    clips: Res<SoundClips>,
    players: Query<(Entity, &ChapterSound)>,
) {
    if !settings.is_changed() {
        return;
    }

    for (entity, sound) in &players {
        if let ChapterSound::Music(track) = sound {
            if *track == settings.music {
                return;
            }
            commands.entity(entity).despawn();
        }
    }

    let (clip, volume) = match settings.music {
        MusicTrack::Silent => return,
        MusicTrack::Menu => (&clips.menu, 0.32),
        MusicTrack::Gameplay => (&clips.gameplay, 0.27),
    };
    commands.spawn((
        ChapterSound::Music(settings.music),
        AudioPlayer(clip.clone()),
        PlaybackSettings {
            paused: settings.paused,
            muted: settings.muted,
            ..PlaybackSettings::LOOP.with_volume(Volume::Linear(volume))
        },
    ));
}

const QUEUE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_CUES_PER_FRAME: usize = 8;

fn maintain_players(
    mut commands: Commands,
    settings: Res<SoundSettings>,
    time: Res<Time<Real>>,
    mut players: Query<(
        Entity,
        &ChapterSound,
        &mut PlaybackSettings,
        Option<&mut AudioSink>,
    )>,
) {
    for (entity, sound, mut pending, sink) in &mut players {
        let paused = settings.paused && sound.pauses();
        if let Some(mut sink) = sink {
            if settings.is_changed() {
                match (paused, sink.is_paused()) {
                    (true, false) => sink.pause(),
                    (false, true) => sink.play(),
                    _ => (),
                }
                if settings.muted != sink.is_muted() {
                    sink.toggle_mute();
                }
            }
        } else if matches!(sound, ChapterSound::Effect { queued_at, .. }
            if time.elapsed().saturating_sub(*queued_at) >= QUEUE_TIMEOUT)
        {
            // Without an output device Bevy never adds a sink or auto-despawns.
            commands.entity(entity).despawn();
        } else if settings.is_changed() {
            pending.paused = paused;
            pending.muted = settings.muted;
        }
    }
}

fn play_cues(
    mut commands: Commands,
    mut messages: MessageReader<SoundCue>,
    clips: Res<SoundClips>,
    settings: Res<SoundSettings>,
    time: Res<Time<Real>>,
) {
    let mut played = 0;
    for &cue in messages.read() {
        // Drain suppressed cues so they don't play on the next frame.
        if settings.muted
            || (settings.paused && cue != SoundCue::MenuClick)
            || played >= MAX_CUES_PER_FRAME
        {
            continue;
        }
        played += 1;
        commands.spawn((
            ChapterSound::Effect {
                cue,
                queued_at: time.elapsed(),
            },
            AudioPlayer(clips.cues[cue.index()].clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.36)),
        ));
    }
}

#[cfg(test)]
mod tests;
