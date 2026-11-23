use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GameState {
    #[default]
    Loading,
    Playing,
    Paused,
}

#[derive(Resource, Default)]
pub(crate) struct LoadingError(pub Option<String>);

#[derive(Component)]
struct Overlay;

pub(crate) struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<LoadingError>()
            .add_systems(OnEnter(GameState::Loading), loading_overlay)
            .add_systems(OnExit(GameState::Loading), remove_overlay)
            .add_systems(Update, animate_loading.run_if(in_state(GameState::Loading)))
            .add_systems(OnEnter(GameState::Paused), (pause_overlay, release_cursor))
            .add_systems(OnExit(GameState::Paused), remove_overlay)
            .add_systems(PreUpdate, toggle_pause.after(InputSystems));
    }
}

fn overlay(commands: &mut Commands, text: &str, background: Color) {
    commands
        .spawn((
            Overlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(1),
            BackgroundColor(background),
        ))
        .with_child((
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(36.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::justify(Justify::Center),
        ));
}

fn loading_overlay(mut commands: Commands) {
    overlay(&mut commands, "Loading...", Color::srgb(0.1, 0.1, 0.15));
}

fn animate_loading(
    time: Res<Time>,
    failure: Res<LoadingError>,
    overlay: Single<&Children, With<Overlay>>,
    mut text: Query<&mut Text>,
) {
    let Ok(mut text) = text.get_mut(overlay[0]) else {
        return;
    };
    let caption = if let Some(failure) = &failure.0 {
        format!("Loading failed\n{failure}")
    } else {
        format!(
            "Loading{}",
            ".".repeat((time.elapsed_secs() * 2.0) as usize % 4)
        )
    };
    if text.0 != caption {
        text.0 = caption;
    }
}

fn pause_overlay(mut commands: Commands) {
    overlay(
        &mut commands,
        "PAUSED\n\nPress Esc to resume",
        Color::srgba(0.0, 0.0, 0.0, 0.7),
    );
}

fn release_cursor(mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursor.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn remove_overlay(mut commands: Commands, overlays: Query<Entity, With<Overlay>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match state.get() {
        GameState::Loading => {}
        GameState::Playing => next.set(GameState::Paused),
        GameState::Paused => next.set(GameState::Playing),
    }
}
