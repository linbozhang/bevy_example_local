use std::collections::HashMap;

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub(super) enum Action {
    Walk,
    Run,
    Jump,
}

#[derive(Deserialize)]
pub(super) struct Clip {
    pub start_row: u32,
    pub frame_count: u32,
    pub frame_time: f32,
    pub directional: bool,
}

#[derive(Deserialize)]
pub(super) struct Character {
    pub name: String,
    pub max_health: f32,
    pub base_move_speed: f32,
    pub run_speed_multiplier: f32,
    pub texture_path: String,
    pub tile_size: u32,
    pub atlas_columns: u32,
    pub atlas_rows: u32,
    pub animations: HashMap<Action, Clip>,
}

#[derive(Asset, TypePath, Deserialize)]
pub(super) struct Roster {
    pub characters: Vec<Character>,
}

impl Roster {
    fn read(bytes: &[u8]) -> Result<Self, bevy::ecs::error::BevyError> {
        let roster: Self = ron::de::from_bytes(bytes)?;
        if roster.characters.is_empty() {
            return Err("characters.ron must contain at least one character".into());
        }
        let positive = |value: f32| value.is_finite() && value > 0.0;
        for entry in &roster.characters {
            let valid = !entry.name.trim().is_empty()
                && !entry.texture_path.is_empty()
                && positive(entry.max_health)
                && positive(entry.base_move_speed)
                && positive(entry.run_speed_multiplier)
                && positive(entry.base_move_speed * entry.run_speed_multiplier)
                && entry.tile_size > 0
                && entry.atlas_columns.checked_mul(entry.tile_size).is_some()
                && entry.atlas_rows.checked_mul(entry.tile_size).is_some()
                && entry.atlas_columns.checked_mul(entry.atlas_rows).is_some()
                && [Action::Walk, Action::Run, Action::Jump]
                    .into_iter()
                    .all(|action| {
                        entry.animations.get(&action).is_some_and(|clip| {
                            (1..=entry.atlas_columns).contains(&clip.frame_count)
                                && positive(clip.frame_time)
                                && clip
                                    .start_row
                                    .checked_add(if clip.directional { 4 } else { 1 })
                                    .is_some_and(|end| end <= entry.atlas_rows)
                        })
                    });
            if !valid {
                return Err(format!("invalid stats or animation atlas for {}", entry.name).into());
            }
        }
        Ok(roster)
    }
}

#[derive(Default, TypePath)]
pub(super) struct RosterLoader;

impl AssetLoader for RosterLoader {
    type Asset = Roster;
    type Settings = ();
    type Error = bevy::ecs::error::BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> Result<Roster, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Roster::read(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["characters.ron"]
    }
}
