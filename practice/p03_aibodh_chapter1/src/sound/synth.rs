//! Original mono PCM clips, generated once and shared by all players.

use std::{f32::consts::TAU, sync::Arc, time::Duration};

use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};

use super::{MusicTrack, SoundCue};

const RATE: SampleRate = SampleRate::new(24_000).unwrap();
const CHANNELS: ChannelCount = ChannelCount::new(1).unwrap();

#[derive(Asset, TypePath)]
pub(super) struct Clip(Arc<[f32]>);

pub(super) struct Samples {
    data: Arc<[f32]>,
    cursor: usize,
}

impl Iterator for Samples {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = *self.data.get(self.cursor)?;
        self.cursor += 1;
        Some(sample)
    }
}

impl Source for Samples {
    fn current_span_len(&self) -> Option<usize> {
        None // This clip uses one format throughout.
    }

    fn channels(&self) -> ChannelCount {
        CHANNELS
    }

    fn sample_rate(&self) -> SampleRate {
        RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.data.len() as f64 / f64::from(RATE.get()),
        ))
    }
}

impl Decodable for Clip {
    type Decoder = Samples;

    fn decoder(&self) -> Self::Decoder {
        Samples {
            data: self.0.clone(),
            cursor: 0,
        }
    }
}

struct Score(Vec<f32>);

impl Score {
    fn new(milliseconds: usize) -> Self {
        Self(vec![0.0; Self::frames(milliseconds)])
    }

    fn frames(milliseconds: usize) -> usize {
        milliseconds * RATE.get() as usize / 1000
    }

    // Each voice fades in and out. Integrating the sweep avoids phase resets.
    fn voice(&mut self, at: usize, length: usize, from: f32, to: f32, gain: f32, grit: f32) {
        let start = Self::frames(at);
        let count = Self::frames(length);
        let mut phase = 0.0_f32;
        for (i, sample) in self.0[start..start + count].iter_mut().enumerate() {
            let progress = i as f32 / (count - 1) as f32;
            let attack = (i as f32 / Self::frames(8) as f32).min(1.0);
            let release = 1.0 - progress;
            let tone =
                phase.sin() + grit * (0.38 * (2.0 * phase).sin() + 0.22 * (3.0 * phase).sin());
            *sample += gain * attack * release * release * tone;
            phase = (phase + TAU * (from + (to - from) * progress) / RATE.get() as f32) % TAU;
        }
    }

    fn note(&mut self, at: usize, length: usize, midi: u8, gain: f32, grit: f32) {
        let hz = 440.0 * 2.0_f32.powf((f32::from(midi) - 69.0) / 12.0);
        self.voice(at, length, hz, hz, gain, grit);
    }

    fn finish(self) -> Clip {
        Clip(self.0.into())
    }
}

pub(super) fn music(track: MusicTrack) -> Clip {
    let (beat, chords, melody, grit) = match track {
        MusicTrack::Menu => (
            500,
            [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]],
            [
                76, 72, 71, 69, 72, 69, 67, 65, 67, 72, 76, 74, 71, 74, 69, 71,
            ],
            0.15,
        ),
        MusicTrack::Gameplay => (
            340,
            [[50, 53, 57], [46, 50, 53], [48, 52, 55], [45, 49, 52]],
            [
                74, 77, 81, 77, 77, 74, 70, 72, 76, 79, 76, 72, 73, 76, 81, 73,
            ],
            0.75,
        ),
        MusicTrack::Silent => unreachable!("silence has no clip"),
    };
    let mut score = Score::new(beat * 16);
    for (bar, [root, third, fifth]) in chords.into_iter().enumerate() {
        let start = bar * beat * 4;
        score.note(start, beat * 4 - 20, root - 12, 0.24, grit);
        for (step, note) in [root, fifth, third, fifth, root, fifth, third, fifth]
            .into_iter()
            .enumerate()
        {
            score.note(start + step * beat / 2, beat / 2 - 12, note, 0.15, grit);
        }
    }
    for (step, note) in melody.into_iter().enumerate() {
        score.note(step * beat, beat - 22, note, 0.26, grit);
    }
    score.finish()
}

pub(super) fn cue(cue: SoundCue) -> Clip {
    let voices: &[(usize, usize, f32, f32, f32, f32)] = match cue {
        SoundCue::Pickup => &[
            (0, 100, 659., 740., 0.45, 0.3),
            (65, 155, 988., 1047., 0.4, 0.2),
        ],
        SoundCue::PlayerCast => &[
            (0, 250, 280., 900., 0.48, 0.8),
            (45, 160, 680., 1300., 0.21, 0.4),
        ],
        SoundCue::EnemyCast => &[
            (0, 320, 540., 120., 0.43, 1.0),
            (35, 230, 315., 90., 0.25, 0.8),
        ],
        SoundCue::Hit => &[
            (0, 130, 175., 52., 0.61, 1.0),
            (0, 36, 970., 340., 0.19, 1.0),
        ],
        SoundCue::PlayerDeath => &[
            (0, 240, 392., 349., 0.40, 0.7),
            (150, 260, 311., 261., 0.42, 0.5),
            (320, 450, 196., 98., 0.49, 0.4),
        ],
        SoundCue::EnemyDeath => &[
            (0, 205, 290., 105., 0.5, 0.9),
            (125, 225, 150., 55., 0.3, 0.9),
        ],
        SoundCue::MenuClick => &[(0, 70, 960., 640., 0.46, 0.5)],
        SoundCue::Jump => &[(0, 170, 240., 500., 0.48, 0.5)],
    };
    let mut score = Score::new(
        voices
            .iter()
            .map(|(at, length, ..)| at + length)
            .max()
            .unwrap(),
    );
    for &(at, length, from, to, gain, grit) in voices {
        score.voice(at, length, from, to, gain, grit);
    }
    score.finish()
}
