use super::loading_screen::LoadingScreenState;
use bevy::app::AppExit;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use mira_game_api::network::{ClientLeave, ReliableCommandChannel};
use lightyear::prelude::*;
use std::time::Duration;

const LEAVE_NOTIFICATION_GRACE_PERIOD: Duration = Duration::from_millis(120);
const NETWORK_DISCONNECT_GRACE_PERIOD: Duration = Duration::from_millis(40);
const LEAVE_MENU_Z_INDEX: i32 = 9_500;

/// Registers the in-game leave confirmation menu.
pub struct LeaveMenuPlugin;

#[derive(Resource, Debug, Default)]
struct LeaveMenuState {
    exit_stage: LeaveExitStage,
    open: bool,
}

/// Tracks the short graceful shutdown sequence used when a player leaves a match.
#[derive(Debug, Default)]
enum LeaveExitStage {
    #[default]
    Idle,
    NotifyingServer(Timer),
    Disconnecting(Timer),
}

/// Describes the next side effect required by the leave shutdown sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaveExitAction {
    None,
    Disconnect,
    Exit,
}

impl LeaveMenuState {
    fn begin_exit(&mut self) -> bool {
        if !matches!(self.exit_stage, LeaveExitStage::Idle) {
            return false;
        }

        self.exit_stage = LeaveExitStage::NotifyingServer(Timer::new(
            LEAVE_NOTIFICATION_GRACE_PERIOD,
            TimerMode::Once,
        ));
        true
    }

    fn advance_exit(&mut self, elapsed: Duration) -> LeaveExitAction {
        let action = match &mut self.exit_stage {
            LeaveExitStage::Idle => LeaveExitAction::None,
            LeaveExitStage::NotifyingServer(timer) => {
                if timer.tick(elapsed).just_finished() {
                    LeaveExitAction::Disconnect
                } else {
                    LeaveExitAction::None
                }
            }
            LeaveExitStage::Disconnecting(timer) => {
                if timer.tick(elapsed).just_finished() {
                    LeaveExitAction::Exit
                } else {
                    LeaveExitAction::None
                }
            }
        };

        match action {
            LeaveExitAction::Disconnect => {
                self.exit_stage = LeaveExitStage::Disconnecting(Timer::new(
                    NETWORK_DISCONNECT_GRACE_PERIOD,
                    TimerMode::Once,
                ));
            }
            LeaveExitAction::Exit => self.exit_stage = LeaveExitStage::Idle,
            LeaveExitAction::None => {}
        }

        action
    }
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
                    advance_leave_exit,
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
            ZIndex(LEAVE_MENU_Z_INDEX),
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
                    TextLayout::justify(Justify::Center),
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
                if state.begin_exit() {
                    send_client_leave(&mut senders);
                    state.open = false;
                }
            }
            LeaveMenuAction::Stay => {
                state.open = false;
            }
        }
    }
}

fn advance_leave_exit(
    time: Res<Time>,
    mut state: ResMut<LeaveMenuState>,
    clients: Query<Entity, (With<Client>, Without<Disconnected>)>,
    mut commands: Commands,
    mut app_exit: MessageWriter<AppExit>,
) {
    match state.advance_exit(time.delta()) {
        LeaveExitAction::None => {}
        LeaveExitAction::Disconnect => disconnect_from_game_server(&clients, &mut commands),
        LeaveExitAction::Exit => {
            app_exit.write(AppExit::Success);
        }
    }
}

fn notify_leave_on_window_close(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut senders: Query<&mut MessageSender<ClientLeave>, With<Client>>,
    clients: Query<Entity, (With<Client>, Without<Disconnected>)>,
    mut commands: Commands,
) {
    if close_requests.read().next().is_none() {
        return;
    }

    send_client_leave(&mut senders);
    disconnect_from_game_server(&clients, &mut commands);
}

fn send_client_leave(senders: &mut Query<&mut MessageSender<ClientLeave>, With<Client>>) {
    for mut sender in senders.iter_mut() {
        sender.send::<ReliableCommandChannel>(ClientLeave);
    }
}

/// Triggers Netcode's explicit disconnect path for every active local game client.
fn disconnect_from_game_server(
    clients: &Query<Entity, (With<Client>, Without<Disconnected>)>,
    commands: &mut Commands,
) {
    for client_entity in clients.iter() {
        commands.trigger(Disconnect {
            entity: client_entity,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leave_exit_waits_for_notification_and_disconnect_packets() {
        let mut state = LeaveMenuState::default();

        assert!(state.begin_exit());
        assert!(!state.begin_exit());
        assert_eq!(
            state.advance_exit(LEAVE_NOTIFICATION_GRACE_PERIOD),
            LeaveExitAction::Disconnect
        );
        assert_eq!(
            state.advance_exit(NETWORK_DISCONNECT_GRACE_PERIOD),
            LeaveExitAction::Exit
        );
    }

    #[test]
    fn idle_leave_exit_does_not_request_side_effects() {
        assert_eq!(
            LeaveMenuState::default().advance_exit(Duration::ZERO),
            LeaveExitAction::None
        );
    }
}
