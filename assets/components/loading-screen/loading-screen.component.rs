use bevy::prelude::*;
use bevy_extended_ui::BeuStore;
use bevy_extended_ui_macros::*;
use serde::Serialize;

#[ui_component]
pub struct LoadingScreenComponent {
    pub template_name: &'static str,
    pub template_file: &'static str,
    pub styles: &'static [&'static str],
}

pub const LOADING_SCREEN_COMPONENT: LoadingScreenComponent = LoadingScreenComponent {
    template_name: "app-loading-screen",
    template_file: "loading-screen.component.html",
    styles: &["loading-screen.component.css"],
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, BeuStore)]
pub struct LoadingScreenStore {
    pub loading_title: String,
    pub loading_subtitle: String,
    pub loading_progress_text: String,
    pub loading_status_text: String,
    pub dark_players: Vec<LoadingPlayerCardStore>,
    pub light_players: Vec<LoadingPlayerCardStore>,
    pub dark_team_count: String,
    pub light_team_count: String,
    pub dark_player_0_initial: String,
    pub dark_player_0_name: String,
    pub dark_player_0_champion: String,
    pub dark_player_0_champion_class: String,
    pub dark_player_0_state: String,
    pub dark_player_1_initial: String,
    pub dark_player_1_name: String,
    pub dark_player_1_champion: String,
    pub dark_player_1_champion_class: String,
    pub dark_player_1_state: String,
    pub dark_player_2_initial: String,
    pub dark_player_2_name: String,
    pub dark_player_2_champion: String,
    pub dark_player_2_champion_class: String,
    pub dark_player_2_state: String,
    pub dark_player_3_initial: String,
    pub dark_player_3_name: String,
    pub dark_player_3_champion: String,
    pub dark_player_3_champion_class: String,
    pub dark_player_3_state: String,
    pub dark_player_4_initial: String,
    pub dark_player_4_name: String,
    pub dark_player_4_champion: String,
    pub dark_player_4_champion_class: String,
    pub dark_player_4_state: String,
    pub light_player_0_initial: String,
    pub light_player_0_name: String,
    pub light_player_0_champion: String,
    pub light_player_0_champion_class: String,
    pub light_player_0_state: String,
    pub light_player_1_initial: String,
    pub light_player_1_name: String,
    pub light_player_1_champion: String,
    pub light_player_1_champion_class: String,
    pub light_player_1_state: String,
    pub light_player_2_initial: String,
    pub light_player_2_name: String,
    pub light_player_2_champion: String,
    pub light_player_2_champion_class: String,
    pub light_player_2_state: String,
    pub light_player_3_initial: String,
    pub light_player_3_name: String,
    pub light_player_3_champion: String,
    pub light_player_3_champion_class: String,
    pub light_player_3_state: String,
    pub light_player_4_initial: String,
    pub light_player_4_name: String,
    pub light_player_4_champion: String,
    pub light_player_4_champion_class: String,
    pub light_player_4_state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LoadingPlayerCardStore {
    pub initial: String,
    pub name: String,
    pub champion: String,
    pub champion_class: String,
    pub state: String,
}

pub struct LoadingScreenComponentPlugin;

impl Plugin for LoadingScreenComponentPlugin {
    fn build(&self, _app: &mut App) {}
}

#[component_init]
pub fn constructor() {}
