use super::loading_screen::LoadingScreenState;
use bevy::app::AppExit;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use game_shared::network::{ClientLeave, ReliableCommandChannel};
use lightyear::prelude::*;

pub struct LeaveMenuPlugin;

#[derive(Resource, Debug, Default)]
struct LeaveMenuState {
    exit_timer: Option<Timer>,
    open: bool,
}

#[derive(Component)]
struct LeaveMenuRoot;

#[derive(Component, Clone, Copy)]
enum LeaveMenuAction {
    Leave,
    Stay,
}

impl Plugin for LeaveMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LeaveMenuState>()
            .add_systems(Startup, spawn_leave_menu)
            .add_systems(
                Update,
                (
                    toggle_leave_menu,
                    sync_leave_menu_visibility,
                    handle_leave_menu_buttons,
                    notify_leave_on_window_close,
                    finish_leave_after_grace,
                ),
            );
    }
}

fn spawn_leave_menu(mut commands: Commands) {
    commands
        .spawn((
            LeaveMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.58)),
            ZIndex(9500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(360),
                    padding: UiRect::all(px(22)),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(18),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.032, 0.048, 0.96)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.14)),
            ))
            .with_children(|dialog| {
                dialog.spawn((
                    Text::new("Leave game?"),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Center),
                ));

                dialog
                    .spawn((Node {
                        display: Display::Flex,
                        column_gap: px(12),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                    .with_children(|actions| {
                        spawn_leave_menu_button(actions, "Leave", LeaveMenuAction::Leave, true);
                        spawn_leave_menu_button(actions, "Stay", LeaveMenuAction::Stay, false);
                    });
            });
        });
}

fn spawn_leave_menu_button(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    action: LeaveMenuAction,
    destructive: bool,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: px(138),
                height: px(42),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(if destructive {
                Color::srgba(0.72, 0.12, 0.08, 0.94)
            } else {
                Color::srgba(0.08, 0.11, 0.16, 0.94)
            }),
            BorderColor::all(if destructive {
                Color::srgba(1.0, 0.38, 0.26, 0.38)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.14)
            }),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont::from_font_size(15.0),
                TextColor(Color::WHITE),
            ));
        });
}

fn toggle_leave_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    loading_screen: Res<LoadingScreenState>,
    mut state: ResMut<LeaveMenuState>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }

    if loading_screen.is_visible() {
        state.open = false;
        return;
    }

    state.open = !state.open;
}

fn sync_leave_menu_visibility(
    state: Res<LeaveMenuState>,
    mut roots: Query<&mut Node, With<LeaveMenuRoot>>,
) {
    if !state.is_changed() {
        return;
    }

    for mut root in &mut roots {
        root.display = if state.open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn handle_leave_menu_buttons(
    mut interactions: Query<(&Interaction, &LeaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<LeaveMenuState>,
    mut senders: Query<&mut MessageSender<ClientLeave>, With<Client>>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            LeaveMenuAction::Leave => {
                send_client_leave(&mut senders);
                state.open = false;
                state.exit_timer = Some(Timer::from_seconds(0.12, TimerMode::Once));
            }
            LeaveMenuAction::Stay => {
                state.open = false;
            }
        }
    }
}

fn finish_leave_after_grace(
    time: Res<Time>,
    mut state: ResMut<LeaveMenuState>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let Some(timer) = state.exit_timer.as_mut() else {
        return;
    };

    if timer.tick(time.delta()).just_finished() {
        state.exit_timer = None;
        app_exit.write(AppExit::Success);
    }
}

fn notify_leave_on_window_close(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut senders: Query<&mut MessageSender<ClientLeave>, With<Client>>,
) {
    if close_requests.read().next().is_none() {
        return;
    }

    send_client_leave(&mut senders);
}

fn send_client_leave(senders: &mut Query<&mut MessageSender<ClientLeave>, With<Client>>) {
    for mut sender in senders.iter_mut() {
        sender.send::<ReliableCommandChannel>(ClientLeave);
    }
}
