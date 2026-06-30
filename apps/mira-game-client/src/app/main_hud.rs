use super::loading_screen::LoadingScreenState;
use super::settings::ClientLaunchSettings;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use game_logic::MiraHudState;

use crate::network::{NetworkPingState, ping_color, ping_text};

const TOP_BAR_WIDTH: u32 = 820;
const TOP_BAR_HEIGHT: u32 = 52;

pub struct MainHudPlugin;

impl Plugin for MainHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_main_hud)
            .add_systems(Update, sync_main_hud);
    }
}

#[derive(Component)]
struct MainHudRoot;

#[derive(Component, Clone, Copy)]
enum HudText {
    Static,
    Status,
    Ping,
    MatchTime,
    ChampionName,
    Health,
    QCooldown,
    WCooldown,
    ECooldown,
    QName,
    WName,
    EName,
}

#[derive(Component)]
struct HudHealthFill;

#[derive(Component)]
struct HudAccentNode;

#[derive(Component)]
struct HudPortrait;

#[derive(Component, Clone, Copy)]
enum AbilitySlot {
    Q,
    W,
    E,
}

#[derive(Component)]
struct HudCooldownFill(AbilitySlot);

#[derive(Resource)]
struct HudImages {
    lira: Handle<Image>,
    ignara: Handle<Image>,
    yuna: Handle<Image>,
    sophia: Handle<Image>,
}

#[derive(Resource)]
struct TopBarShapeImage {
    handle: Handle<Image>,
}

fn spawn_main_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ui_images: ResMut<Assets<Image>>,
) {
    let images = HudImages {
        lira: asset_server.load("characters/lira.png"),
        ignara: asset_server.load("characters/ignara.png"),
        yuna: asset_server.load("characters/yuna.png"),
        sophia: asset_server.load("characters/sophia.png"),
    };
    let top_bar_shape = TopBarShapeImage {
        handle: ui_images.add(top_bar_shape_image(accent_fallback())),
    };

    commands.spawn((
        MainHudRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            display: Display::Flex,
            ..default()
        },
        Pickable::IGNORE,
        ZIndex(10),
        children![
            top_bar(top_bar_shape.handle.clone()),
            bottom_hud(images.lira.clone())
        ],
    ));

    commands.insert_resource(images);
    commands.insert_resource(top_bar_shape);
}

fn top_bar(shape_image: Handle<Image>) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: percent(50),
            width: px(820),
            height: px(52),
            display: Display::Flex,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(78), px(6)),
            ..default()
        },
        UiTransform::from_translation(Val2::px(-410.0, 0.0)),
        children![
            top_bar_shape(shape_image),
            top_segment(
                "K / D / A",
                "0 / 0 / 0",
                HudText::Static,
                AlignItems::FlexStart
            ),
            top_segment(
                "Practice Arena",
                "LIVE",
                HudText::Status,
                AlignItems::Center
            ),
            top_time_segment(),
        ],
    )
}

fn top_bar_shape(shape_image: Handle<Image>) -> impl Bundle {
    (
        ImageNode {
            image: shape_image,
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            right: px(0),
            width: percent(100),
            height: percent(100),
            ..default()
        },
    )
}

fn top_segment(
    label: &'static str,
    value: &'static str,
    marker: HudText,
    align_items: AlignItems,
) -> impl Bundle {
    (
        Node {
            width: px(220),
            height: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items,
            ..default()
        },
        children![label_text(label, 10.0), value_text(value, 15.0, marker)],
    )
}

fn top_time_segment() -> impl Bundle {
    (
        Node {
            width: px(220),
            height: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::FlexEnd,
            row_gap: px(2),
            ..default()
        },
        children![
            label_text("Ping / Time", 10.0),
            (
                Node {
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                },
                children![
                    text("0ms", 13.0, Color::srgb_u8(0x2B, 0xB8, 0x61), HudText::Ping),
                    value_text("00:00", 15.0, HudText::MatchTime),
                ],
            ),
        ],
    )
}

fn bottom_hud(default_portrait: Handle<Image>) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: percent(50),
            width: px(760),
            height: px(132),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            ..default()
        },
        UiTransform::from_translation(Val2::px(-380.0, 0.0)),
        children![(
            Node {
                width: percent(100),
                height: percent(100),
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(8),
                padding: UiRect::all(px(8)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(hud_bg()),
            BorderColor::all(line()),
            children![champion_panel(default_portrait), spells_panel()],
        )],
    )
}

fn champion_panel(default_portrait: Handle<Image>) -> impl Bundle {
    (
        Node {
            width: px(314),
            height: percent(100),
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: UiRect::all(px(9)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(hud_bg_raised()),
        BorderColor::all(line_soft()),
        children![
            portrait(default_portrait),
            (
                Node {
                    width: px(152),
                    min_width: px(0),
                    flex_grow: 1.0,
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    ..default()
                },
                children![
                    (
                        Node {
                            height: px(22),
                            display: Display::Flex,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(8),
                            ..default()
                        },
                        children![
                            text("LIRA", 15.0, ink(), HudText::ChampionName),
                            status_pill(),
                        ],
                    ),
                    text("100/100", 13.0, Color::WHITE, HudText::Health),
                    health_track(),
                    mana_track(),
                ],
            ),
            stat_stack(),
        ],
    )
}

fn portrait(default_portrait: Handle<Image>) -> impl Bundle {
    (
        HudAccentNode,
        Node {
            width: px(84),
            height: px(84),
            min_width: px(84),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(accent_fallback()),
        BorderColor::all(line_portrait()),
        children![(
            HudPortrait,
            ImageNode::new(default_portrait),
            Node {
                width: px(84),
                height: px(84),
                ..default()
            },
        )],
    )
}

fn status_pill() -> impl Bundle {
    (
        Node {
            width: px(48),
            height: px(20),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(hud_bg_deep()),
        BorderColor::all(line_soft()),
        children![text("LIVE", 10.0, muted_strong(), HudText::Status)],
    )
}

fn health_track() -> impl Bundle {
    (
        Node {
            width: px(148),
            height: px(14),
            display: Display::Flex,
            overflow: Overflow::clip(),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(hud_bg_deep()),
        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.69)),
        children![(
            HudHealthFill,
            Node {
                width: percent(100),
                height: percent(100),
                min_width: px(0),
                ..default()
            },
            BackgroundColor(health()),
        )],
    )
}

fn mana_track() -> impl Bundle {
    (
        Node {
            width: px(148),
            height: px(10),
            overflow: Overflow::clip(),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(hud_bg_deep()),
        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.66)),
        children![(
            Node {
                width: px(148),
                height: percent(100),
                ..default()
            },
            BackgroundColor(mana()),
        )],
    )
}

fn stat_stack() -> impl Bundle {
    (Node {
        width: px(0),
        height: px(58),
        display: Display::None,
        ..default()
    },)
}

fn spells_panel() -> impl Bundle {
    (
        Node {
            flex_grow: 1.0,
            height: percent(100),
            display: Display::Flex,
            align_items: AlignItems::FlexEnd,
            justify_content: JustifyContent::Center,
            column_gap: px(8),
            padding: UiRect::axes(px(10), px(9)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(hud_bg_raised()),
        BorderColor::all(line_soft()),
        children![
            ability_slot(
                AbilitySlot::Q,
                "Q",
                "PIERCING BOLT",
                Color::srgba(0.22, 0.74, 0.97, 1.0),
                HudText::QName,
                HudText::QCooldown
            ),
            ability_slot(
                AbilitySlot::W,
                "W",
                "ARC BURST",
                Color::srgba(0.97, 0.79, 0.28, 1.0),
                HudText::WName,
                HudText::WCooldown
            ),
            ability_slot(
                AbilitySlot::E,
                "E",
                "ORBIT MISSILES",
                Color::srgba(0.66, 0.33, 0.97, 1.0),
                HudText::EName,
                HudText::ECooldown
            ),
        ],
    )
}

fn ability_slot(
    slot: AbilitySlot,
    key: &'static str,
    name: &'static str,
    icon_color: Color,
    name_marker: HudText,
    cooldown_marker: HudText,
) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Relative,
            width: px(76),
            height: px(98),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.067, 0.09, 0.137, 1.0)),
        BorderColor::all(Color::srgba(0.85, 0.74, 0.44, 0.33)),
        children![
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    left: px(12),
                    width: px(52),
                    height: px(52),
                    min_width: px(52),
                    ..default()
                },
                BackgroundColor(icon_color),
            ),
            (
                HudCooldownFill(slot),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: percent(100),
                    width: percent(100),
                    height: percent(0),
                    min_width: px(0),
                    ..default()
                },
                BackgroundColor(accent_fallback().with_alpha(0.74)),
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(28),
                    left: px(0),
                    width: percent(100),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![text("", 16.0, Color::WHITE, cooldown_marker)],
            ),
            (
                HudAccentNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(2),
                    bottom: px(2),
                    width: px(20),
                    height: px(18),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(accent_fallback()),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.26)),
                children![text(key, 13.0, accent_foreground(), HudText::Static)],
            ),
            (
                Node {
                    width: px(70),
                    height: px(28),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(6)),
                    ..default()
                },
                children![text(name, 10.0, muted_strong(), name_marker)],
            ),
        ],
    )
}

fn sync_main_hud(
    time: Res<Time>,
    hud_state: Res<MiraHudState>,
    launch_settings: Res<ClientLaunchSettings>,
    loading_state: Res<LoadingScreenState>,
    network_ping: Res<NetworkPingState>,
    images: Res<HudImages>,
    top_bar_shape: Res<TopBarShapeImage>,
    mut ui_images: ResMut<Assets<Image>>,
    mut last_top_bar_accent: Local<Option<Color>>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut layout_nodes: ParamSet<(
        Query<&mut Node, With<MainHudRoot>>,
        Query<&mut Node, With<HudHealthFill>>,
        Query<(&HudCooldownFill, &mut Node, &mut BackgroundColor)>,
        Query<(&mut BackgroundColor, Option<&mut BorderColor>), With<HudAccentNode>>,
    )>,
    mut portrait: Single<&mut ImageNode, With<HudPortrait>>,
) {
    for mut root in &mut layout_nodes.p0() {
        root.display = if loading_state.is_visible() {
            Display::None
        } else {
            Display::Flex
        };
    }

    let status = if launch_settings.dev_preview {
        "Dev"
    } else if hud_state.alive {
        "LIVE"
    } else {
        "DEAD"
    };
    let health_text = format!("{}/{}", hud_state.health_current, hud_state.health_max);
    let match_time = match_time_text(time.elapsed_secs());

    let ping_text = ping_text(&network_ping);
    let ping_color = ping_color(&network_ping);

    for (kind, mut text, mut color) in &mut texts {
        text.0 = match kind {
            HudText::Static => continue,
            HudText::Status => status.to_string(),
            HudText::Ping => {
                *color = TextColor(ping_color);
                ping_text.clone()
            }
            HudText::MatchTime => match_time.clone(),
            HudText::ChampionName => hud_state.champion_name.to_ascii_uppercase(),
            HudText::Health => health_text.clone(),
            HudText::QCooldown => cooldown_text(hud_state.q_cooldown_remaining),
            HudText::WCooldown => cooldown_text(hud_state.w_cooldown_remaining),
            HudText::ECooldown => cooldown_text(hud_state.e_cooldown_remaining),
            HudText::QName => hud_state.q_name.to_ascii_uppercase(),
            HudText::WName => hud_state.w_name.to_ascii_uppercase(),
            HudText::EName => hud_state.e_name.to_ascii_uppercase(),
        };
    }

    for mut health_fill in &mut layout_nodes.p1() {
        health_fill.width = percent(hud_state.health_percent.clamp(0.0, 100.0));
    }

    let accent = launch_settings.accent_color_bevy();
    if last_top_bar_accent.as_ref() != Some(&accent) {
        if let Some(image) = ui_images.get_mut(top_bar_shape.handle.id()) {
            *image = top_bar_shape_image(accent);
        }
        *last_top_bar_accent = Some(accent);
    }

    for (mut background, border) in &mut layout_nodes.p3() {
        *background = BackgroundColor(accent);
        if let Some(mut border) = border {
            border.set_all(accent);
        }
    }

    for (fill, mut node, mut background) in &mut layout_nodes.p2() {
        let ready_percent = match fill.0 {
            AbilitySlot::Q => hud_state.q_ready_percent,
            AbilitySlot::W => hud_state.w_ready_percent,
            AbilitySlot::E => hud_state.e_ready_percent,
        }
        .clamp(0.0, 100.0);

        node.top = percent(ready_percent);
        node.height = percent(100.0 - ready_percent);
        *background = BackgroundColor(accent.with_alpha(0.74));
    }

    portrait.image = champion_portrait(&images, &hud_state.champion_name).clone();
}

fn champion_portrait<'a>(images: &'a HudImages, champion_name: &str) -> &'a Handle<Image> {
    match champion_name.trim().to_ascii_lowercase().as_str() {
        "ignara" => &images.ignara,
        "yuna" => &images.yuna,
        "sophia" => &images.sophia,
        _ => &images.lira,
    }
}

fn top_bar_shape_image(accent: Color) -> Image {
    let width = TOP_BAR_WIDTH;
    let height = TOP_BAR_HEIGHT;
    let fill = rgba_bytes(hud_bg());
    let border = rgba_bytes(accent);
    let mut data = vec![0; (width * height * 4) as usize];

    for y in 0..height {
        let t = y as f32 / (height.saturating_sub(1)) as f32;
        let side_inset = 34.0 + (70.0 - 34.0) * t;
        let left = side_inset;
        let right = width as f32 - 1.0 - side_inset;

        for x in 0..width {
            let x_pos = x as f32;
            if x_pos < left || x_pos > right {
                continue;
            }

            let is_border = y < 2 || y >= height - 2 || x_pos < left + 2.0 || x_pos > right - 2.0;
            let pixel = if is_border { border } else { fill };
            let offset = ((y * width + x) * 4) as usize;
            data[offset..offset + 4].copy_from_slice(&pixel);
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn rgba_bytes(color: Color) -> [u8; 4] {
    let color = color.to_srgba();
    [
        channel_to_byte(color.red),
        channel_to_byte(color.green),
        channel_to_byte(color.blue),
        channel_to_byte(color.alpha),
    ]
}

fn channel_to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn label_text(value: &'static str, size: f32) -> impl Bundle {
    text(value, size, muted(), HudText::Static)
}

fn value_text(value: &'static str, size: f32, marker: HudText) -> impl Bundle {
    text(value, size, ink(), marker)
}

fn text(value: &'static str, size: f32, color: Color, marker: HudText) -> impl Bundle {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::new_with_justify(Justify::Center),
        marker,
    )
}

fn match_time_text(elapsed_seconds: f32) -> String {
    let total_seconds = elapsed_seconds.max(0.0).floor() as u64;
    let hours = total_seconds / 3600;
    let minutes = total_seconds % 3600 / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn cooldown_text(remaining_seconds: f32) -> String {
    if remaining_seconds <= 0.05 {
        String::new()
    } else {
        format!("{}s", remaining_seconds.max(0.0).ceil() as u32)
    }
}

fn hud_bg() -> Color {
    Color::srgba(0.031, 0.043, 0.063, 0.91)
}

fn hud_bg_raised() -> Color {
    Color::srgba(0.071, 0.09, 0.133, 0.94)
}

fn hud_bg_deep() -> Color {
    Color::srgba(0.012, 0.02, 0.039, 1.0)
}

fn line() -> Color {
    Color::srgba(0.78, 0.66, 0.35, 0.40)
}

fn line_soft() -> Color {
    Color::srgba(1.0, 1.0, 1.0, 0.11)
}

fn line_portrait() -> Color {
    Color::srgba(0.85, 0.74, 0.44, 0.60)
}

fn health() -> Color {
    Color::srgba(0.21, 0.82, 0.44, 1.0)
}

fn mana() -> Color {
    Color::srgba(0.18, 0.54, 1.0, 1.0)
}

fn ink() -> Color {
    Color::srgba(0.93, 0.95, 0.97, 1.0)
}

fn muted() -> Color {
    Color::srgba(0.60, 0.64, 0.70, 1.0)
}

fn muted_strong() -> Color {
    Color::srgba(0.78, 0.82, 0.86, 1.0)
}

fn accent_fallback() -> Color {
    Color::srgba(0.95, 0.77, 0.36, 1.0)
}

fn accent_foreground() -> Color {
    Color::srgba(0.063, 0.071, 0.086, 1.0)
}
