use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::{Activate, Button},
};

use crate::game_state::GameState;

use super::{
    begin_load, capture,
    storage::{SLOT_COUNT, Slots},
};

const SLOT_KEYS: [KeyCode; SLOT_COUNT] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Save,
    Load,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Open(Mode),
    Select(usize),
    Back,
}

#[derive(Resource, Default)]
pub(crate) struct SaveMenu {
    mode: Option<Mode>,
    overwrite: Option<usize>,
    notice: String,
    clicked: Option<Action>,
}

impl SaveMenu {
    pub(crate) fn open_load(&mut self) {
        self.open(Mode::Load);
    }

    pub(crate) fn is_slot_menu_open(&self) -> bool {
        self.mode.is_some()
    }

    pub(super) fn set_notice(&mut self, text: String) {
        self.notice = text;
    }

    fn open(&mut self, mode: Mode) {
        *self = Self {
            mode: Some(mode),
            ..default()
        };
    }
}

#[derive(Component)]
pub(super) struct MenuRoot;

#[derive(Component)]
pub(super) struct MenuButton;

fn shortcut(keys: &ButtonInput<KeyCode>, menu: &SaveMenu, state: GameState) -> Option<Action> {
    if menu.is_slot_menu_open() {
        if keys.just_pressed(KeyCode::Escape) {
            return Some(Action::Back);
        }
        SLOT_KEYS
            .iter()
            .position(|key| keys.just_pressed(*key))
            .map(Action::Select)
    } else if state == GameState::Paused && keys.just_pressed(KeyCode::KeyS) {
        Some(Action::Open(Mode::Save))
    } else if matches!(state, GameState::Paused | GameState::GameOver)
        && keys.just_pressed(KeyCode::KeyL)
    {
        Some(Action::Open(Mode::Load))
    } else {
        None
    }
}

pub(super) fn menu_input(world: &mut World) {
    let state = *world.resource::<State<GameState>>().get();
    if matches!(state, GameState::Playing | GameState::Loading) {
        return;
    }
    let menu = world.resource::<SaveMenu>();
    let Some(action) =
        shortcut(world.resource::<ButtonInput<KeyCode>>(), menu, state).or(menu.clicked)
    else {
        return;
    };
    world.resource_mut::<SaveMenu>().clicked = None;

    match action {
        Action::Open(Mode::Save) if state == GameState::Paused => {
            world.resource_mut::<SaveMenu>().open(Mode::Save)
        }
        Action::Open(Mode::Load) => world.resource_mut::<SaveMenu>().open_load(),
        Action::Back => *world.resource_mut::<SaveMenu>() = SaveMenu::default(),
        Action::Select(slot) => {
            let Some(mode) = world.resource::<SaveMenu>().mode else {
                return;
            };
            if mode == Mode::Save && state != GameState::Paused {
                return;
            }
            if let Err(error) = select_slot(world, mode, slot) {
                world.resource_mut::<SaveMenu>().set_notice(error);
            }
        }
        Action::Open(Mode::Save) => {}
    }
}

fn select_slot(world: &mut World, mode: Mode, slot: usize) -> Result<(), String> {
    let slots = Slots::beside_executable().map_err(|error| error.to_string())?;
    let previous = slots.read(slot);
    match mode {
        Mode::Load => {
            let saved = previous?.ok_or("This slot is empty.")?;
            begin_load(world, saved)?;
            *world.resource_mut::<SaveMenu>() = SaveMenu::default();
        }
        Mode::Save => {
            if !matches!(previous, Ok(None)) && world.resource::<SaveMenu>().overwrite != Some(slot)
            {
                let mut menu = world.resource_mut::<SaveMenu>();
                menu.overwrite = Some(slot);
                menu.set_notice(format!("Choose slot {} again to overwrite it.", slot + 1));
                return Ok(());
            }
            let mut menu = world.resource_mut::<SaveMenu>();
            menu.overwrite = None;
            let snapshot = capture(world)?;
            slots.write(slot, &snapshot)?;
            world
                .resource_mut::<SaveMenu>()
                .set_notice(format!("Saved to slot {}.", slot + 1));
        }
    }
    Ok(())
}

pub(super) fn rebuild(
    mut commands: Commands,
    state: Res<State<GameState>>,
    menu: Res<SaveMenu>,
    roots: Query<Entity, With<MenuRoot>>,
    mut last_state: Local<Option<GameState>>,
) {
    if *last_state == Some(*state.get()) && !menu.is_changed() {
        return;
    }
    *last_state = Some(*state.get());
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if matches!(state.get(), GameState::Playing | GameState::Loading) {
        return;
    }
    if let Some(mode) = menu.mode {
        spawn_slots(&mut commands, &menu, mode);
    } else if matches!(state.get(), GameState::Paused | GameState::GameOver) {
        commands
            .spawn((
                MenuRoot,
                GlobalZIndex(10),
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(66.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
            ))
            .with_children(|parent| {
                if *state.get() == GameState::Paused {
                    button(parent, "Save Game  [S]", Action::Open(Mode::Save), false);
                }
                button(parent, "Load Game  [L]", Action::Open(Mode::Load), false);
                if !menu.notice.is_empty() {
                    parent.spawn(text(&menu.notice, 17.0, true));
                }
            });
    }
}

fn spawn_slots(commands: &mut Commands, menu: &SaveMenu, mode: Mode) {
    let slots = Slots::beside_executable();
    commands
        .spawn((
            MenuRoot,
            GlobalZIndex(20),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.08, 0.14)),
        ))
        .with_children(|parent| {
            parent.spawn(text(
                match mode {
                    Mode::Save => "Save Game",
                    Mode::Load => "Load Game",
                },
                32.0,
                true,
            ));
            for slot in 0..SLOT_COUNT {
                let entry = slots
                    .as_ref()
                    .map_err(ToString::to_string)
                    .and_then(|slots| slots.read(slot));
                let disabled = mode == Mode::Load && !matches!(entry, Ok(Some(_)));
                let detail = match entry {
                    Ok(Some(saved)) => format!(
                        "{} · {:.0}/{:.0} HP · seed {}",
                        saved.player.character,
                        saved.player.health.current,
                        saved.player.health.max,
                        saved.seed
                    ),
                    Ok(None) => "Empty".into(),
                    Err(error) => format!("Unavailable: {error}"),
                };
                button(
                    parent,
                    &format!("{}  —  {detail}", slot + 1),
                    Action::Select(slot),
                    disabled,
                );
            }
            button(parent, "Back  [Esc]", Action::Back, false);
            let notice = if !menu.notice.is_empty() {
                &menu.notice
            } else {
                "Choose a slot with the mouse or keys 1–5."
            };
            parent.spawn(text(notice, 17.0, true));
        });
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: Action, disabled: bool) {
    let mut button = parent.spawn((
        MenuButton,
        Button,
        Hovered::default(),
        Node {
            width: Val::Percent(90.0),
            max_width: Val::Px(640.0),
            min_height: Val::Px(40.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(button_color(false, false, disabled)),
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_child(text(label, 18.0, !disabled)).observe(
        move |_: On<Activate>, mut menu: ResMut<SaveMenu>| {
            menu.clicked = Some(action);
        },
    );
}

fn text(label: &str, size: f32, bright: bool) -> impl Bundle {
    (
        Text::new(label),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(if bright {
            Color::WHITE
        } else {
            Color::srgb(0.58, 0.62, 0.68)
        }),
        TextLayout::justify(Justify::Center),
        Pickable::IGNORE,
    )
}

pub(super) fn color_buttons(
    mut buttons: Query<
        (
            &Hovered,
            Has<Pressed>,
            Has<InteractionDisabled>,
            &mut BackgroundColor,
        ),
        With<MenuButton>,
    >,
) {
    for (hovered, pressed, disabled, mut background) in &mut buttons {
        background.set_if_neq(BackgroundColor(button_color(
            hovered.get(),
            pressed,
            disabled,
        )));
    }
}

fn button_color(hovered: bool, pressed: bool, disabled: bool) -> Color {
    match (hovered, pressed, disabled) {
        (_, _, true) => Color::srgb(0.13, 0.15, 0.19),
        (_, true, false) => Color::srgb(0.27, 0.42, 0.62),
        (true, _, false) => Color::srgb(0.20, 0.31, 0.51),
        _ => Color::srgb(0.13, 0.22, 0.35),
    }
}

/// State transitions must not carry an open slot dialog into the next screen.
pub(super) fn reset_menu(mut menu: ResMut<SaveMenu>) {
    *menu = SaveMenu::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_shortcut_only_works_from_pause() {
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyS);
        let menu = SaveMenu::default();
        assert_eq!(
            shortcut(&keys, &menu, GameState::Paused),
            Some(Action::Open(Mode::Save))
        );
        assert_eq!(shortcut(&keys, &menu, GameState::GameOver), None);
    }

    #[test]
    fn slot_shortcuts_and_back_take_precedence_in_dialog() {
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Digit5);
        keys.press(KeyCode::KeyS);
        let mut menu = SaveMenu::default();
        menu.open_load();
        assert_eq!(
            shortcut(&keys, &menu, GameState::Paused),
            Some(Action::Select(4))
        );
        keys.press(KeyCode::Escape);
        assert_eq!(
            shortcut(&keys, &menu, GameState::Paused),
            Some(Action::Back)
        );
    }
}
