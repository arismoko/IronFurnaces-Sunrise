//! BlockIronFurnaceScreenBase and BlockWirelessEnergyHeaterScreenBase: the furnace's three pages (furnace, factory,
//! generator), the augment page, the config tab (auto input and output, the sides, redstone), and the transmitter's
//! screen; drawn and clicked on the client from the menu's data, acted on by the server (`menu_button`).

use crate::furnace::{self, *};
use crate::util::*;
use crate::{config, with_state};
use mod_api::*;

// Menu buttons.
pub(crate) const B_SPLIT: i32 = 0;
pub(crate) const B_AUGMENT_GUI: i32 = 1;
pub(crate) const B_AUTO_IN: i32 = 2;
pub(crate) const B_AUTO_OUT: i32 = 3;
pub(crate) const B_REDSTONE: i32 = 4; // + 0..2
pub(crate) const B_RESET_SIDES: i32 = 7;
pub(crate) const B_SIDE: i32 = 10; // + screen side * 5 + setting

/// The container's slot positions (BlockIronFurnaceContainerBase).
const AT: [[f32; 2]; SLOTS] = [
    [56.0, 17.0], [56.0, 53.0], [116.0, 35.0], [26.0, 35.0], [80.0, 35.0], [134.0, 35.0], [56.0, 40.0],
    [28.0, 6.0], [49.0, 6.0], [70.0, 6.0], [91.0, 6.0], [112.0, 6.0], [133.0, 6.0],
    [28.0, 55.0], [49.0, 55.0], [70.0, 55.0], [91.0, 55.0], [112.0, 55.0], [133.0, 55.0],
];

/// The Slot subclasses' isActive: which slots a page shows.
pub(crate) fn slot_shown(augment_page: bool, mode: i32, tier: i32, slot: usize) -> bool {
    if augment_page {
        return (AUGMENT_RED..=AUGMENT_BLUE).contains(&slot);
    }
    match mode {
        0 => matches!(slot, INPUT | FUEL | OUTPUT),
        1 => {
            let range = factory_range(tier);
            ((7..=12).contains(&slot) && range.contains(&(slot - 7))) || ((13..=18).contains(&slot) && range.contains(&(slot - 13)))
        }
        _ => slot == GENERATOR_FUEL,
    }
}

/// The furnace's page and mode, from its saved compound (the server's, or the client's copy).
fn page_of(host: &dyn Host, pos: Pos) -> Option<(bool, i32)> {
    let level = host.level(host.dimension())?;
    let data = host.block_entity(level, pos)?.data(host)?;
    Some((get_i(&data, "AugmentGUI", 0) != 0, get_i(&data, "hz_mode", 0)))
}

/// The client's screen for one opening of a furnace's or the transmitter's menu (`register_menu_screen`): the layout
/// for the block it opened on, drawn, clicked and hovered from the menu's live data.
pub(crate) fn create(host: &dyn ClientHost, open: MenuScreenOpen<'_>) -> Option<MenuScreen> {
    let block = open.block?.to_owned();
    let menu = open.menu.to_owned();
    let screen = MenuScreen::new(layout(host, &block, open.pos)?);
    let (on_draw, on_click, on_tooltip) = (block.clone(), block.clone(), block);
    Some(
        screen
            .with_draw(move |host, frame, ctx| {
                relayout(host, &menu, &on_draw, frame.data);
                draw(host, &on_draw, frame.data, ctx, frame.origin)
            })
            .with_click(move |host, frame, right| click(host, &on_click, frame.data, frame.at, right))
            .with_tooltip(move |host, frame, _slot, lines| {
                let mut own = Vec::new();
                tooltip(host, &on_tooltip, frame.data, frame.at, &mut own);
                lines.extend(own.into_iter().map(|l| (Text::literal(l), u32::MAX)));
            }),
    )
}

/// The screen's layout when it opens: its size, and where each slot of the page shown sits. A slot the page doesn't
/// show has no place, so it is neither drawn nor clicked.
pub(crate) fn layout(host: &dyn Host, block: &str, pos: Option<Pos>) -> Option<MenuScreenLayout> {
    if is_heater(block) {
        return Some(MenuScreenLayout {
            width: 176.0,
            height: 166.0,
            // BlockWirelessEnergyHeaterContainer: SlotHeater(te, 0, 80, 37).
            slots: vec![Some(slot_at([80.0, 37.0]))],
            inventory_y: 84.0,
            inventory_x: 8.0,
            own_labels: true,
            own_background: true,
            ..MenuScreenLayout::default()
        });
    }
    if !is_furnace(block) {
        return None;
    }
    let (augment_page, mode) = pos.and_then(|pos| page_of(host, pos)).unwrap_or((false, 0));
    let tier = config::tier(host, block);
    let slots = (0..SLOTS).map(|i| slot_shown(augment_page, mode, tier, i).then_some(slot_at(AT[i]))).collect();
    Some(MenuScreenLayout {
        width: 176.0,
        height: 166.0,
        slots,
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
        ..MenuScreenLayout::default()
    })
}

/// The Slot subclasses' isActive, read each frame: when the page or the mode changes (the menu's live data), the
/// slots it shows move in and the others leave from the next frame.
fn relayout(host: &dyn ClientHost, menu: &str, block: &str, data: &[i32]) {
    let (Some(&augment_page), Some(&mode)) = (data.get(D_AUGMENT_GUI), data.get(D_MODE)) else { return };
    if !is_furnace(block) {
        return;
    }
    let tier = config::tier(host, block);
    for (i, at) in AT.iter().enumerate() {
        let _ = host.set_screen_slot(menu, i, slot_shown(augment_page != 0, mode, tier, i).then_some(*at));
    }
}

#[allow(clippy::too_many_arguments)] // `GuiGraphics.blit`'s own arguments.
fn sprite(draw: &mut dyn DrawContext, texture: &str, o: [f32; 2], x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
    if w > 0.0 && h > 0.0 {
        draw.sprite(texture, [u, v, w, h], [o[0] + x, o[1] + y, w, h], None);
    }
}

const WIDGETS: &str = "ironfurnaces:gui/widgets";
const FACTORY: &str = "ironfurnaces:gui/furnace_factory";
const GENERATOR: &str = "ironfurnaces:gui/furnace_generator";
const AUGMENTS: &str = "ironfurnaces:gui/augment";
const ENERGY: &str = "ironfurnaces:gui/energy";

/// The furnace page's texture (the subclasses' GUI).
fn furnace_texture(block: &str) -> &'static str {
    match block {
        "ironfurnaces:netherite_furnace" => "ironfurnaces:gui/furnace_netherite",
        "ironfurnaces:allthemodium_furnace" => "ironfurnaces:gui/furnace_allthemodium",
        "ironfurnaces:vibranium_furnace" => "ironfurnaces:gui/furnace_vibranium",
        "ironfurnaces:unobtainium_furnace" => "ironfurnaces:gui/furnace_unobtainium",
        _ => "ironfurnaces:gui/furnace",
    }
}

/// FurnaceGuiButton.hovering.
fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] <= r[0] + r[2] && at[1] >= r[1] && at[1] <= r[1] + r[3]
}

const R_AUGMENT: [f32; 4] = [161.0, 4.0, 11.0, 11.0];
const R_SPLIT: [f32; 4] = [9.0, 56.0, 14.0, 14.0];
const R_AUTO_IN: [f32; 4] = [-47.0, 8.0, 14.0, 14.0];
const R_AUTO_OUT: [f32; 4] = [-29.0, 8.0, 14.0, 14.0];
const R_RED: [[f32; 4]; 3] = [[-47.0, 66.0, 14.0, 14.0], [-31.0, 66.0, 14.0, 14.0], [-15.0, 66.0, 14.0, 14.0]];
// Bottom, top, front, back, left, right.
const R_SIDES: [[f32; 4]; 6] =
    [[-32.0, 51.0, 10.0, 10.0], [-32.0, 27.0, 10.0, 10.0], [-32.0, 39.0, 10.0, 10.0], [-20.0, 51.0, 10.0, 10.0], [-44.0, 39.0, 10.0, 10.0], [-20.0, 39.0, 10.0, 10.0]];

/// FurnaceGuiButton.render with a hover and an enabled texture part (the same part, as the Java buttons have).
fn button(draw: &mut dyn DrawContext, o: [f32; 2], r: [f32; 4], mouse: [f32; 2], hover: Option<[f32; 2]>, enabled: Option<[f32; 2]>) {
    if inside(r, mouse)
        && let Some(uv) = hover
    {
        sprite(draw, WIDGETS, o, r[0], r[1], uv[0], uv[1], r[2], r[3]);
    }
    if let Some(uv) = enabled {
        sprite(draw, WIDGETS, o, r[0], r[1], uv[0], uv[1], r[2], r[3]);
    }
}

fn scaled(value: i32, total: i32, pixels: i32) -> i32 {
    if total > 0 && value > 0 { (i64::from(value) * i64::from(pixels) / i64::from(total)) as i32 } else { 0 }
}

/// The furnace's name as its screen shows it: the Rainbow Furnace's in colours that change every 20 frames.
fn title(host: &dyn ClientHost, block: &str) -> Vec<(String, u32)> {
    let name = block.strip_prefix("ironfurnaces:").unwrap_or(block);
    let text = host
        .translate(&format!("container.ironfurnaces.{name}"))
        .or_else(|| host.translate(&format!("block.ironfurnaces.{name}")))
        .unwrap_or_else(|| name.to_owned());
    if !is_rainbow(block) {
        return vec![(text, 0)];
    }
    // ItemMillionFurnace.getIDRandom over ChatFormatting: red, yellow, green, aqua, blue, light purple.
    const COLOURS: [u32; 6] = [0xFFFF5555, 0xFFFFFF55, 0xFF55FF55, 0xFF55FFFF, 0xFF5555FF, 0xFFFF55FF];
    let seed = with_state(|s| {
        s.title_timer += 1;
        if s.title_timer % 20 == 0 || s.title_seed == 0 {
            s.title_seed = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos()) as u64) | 1;
        }
        s.title_seed
    });
    let mut x = seed;
    text.chars()
        .map(|c| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (c.to_string(), COLOURS[(x % 6) as usize])
        })
        .collect()
}

pub(crate) fn draw(host: &dyn ClientHost, block: &str, data: &[i32], draw: &mut dyn DrawContext, origin: [f32; 2]) {
    let o = origin;
    let mouse = with_state(|s| s.mouse);
    if is_heater(block) {
        draw_heater(host, data, draw, o, mouse);
        return;
    }
    if !is_furnace(block) || data.len() < DATA_SLOTS {
        return;
    }
    let config_open = with_state(|s| s.config_open);
    let m = data[D_MODE];
    let augment_page = data[D_AUGMENT_GUI] != 0;
    let tier = data[D_TIER];
    // bg.
    let texture = if augment_page {
        AUGMENTS
    } else if m == 1 {
        FACTORY
    } else if m == 2 {
        GENERATOR
    } else {
        furnace_texture(block)
    };
    sprite(draw, texture, o, 0.0, 0.0, 0.0, 0.0, 176.0, 166.0);
    if !augment_page {
        match m {
            0 => {
                if data[D_BURN] > 0 {
                    let i = scaled(data[D_BURN], data[D_BURN_TOTAL].max(1), 13) as f32;
                    sprite(draw, texture, o, 56.0, 36.0 + 12.0 - i, 176.0, 12.0 - i, 14.0, i + 1.0);
                }
                let i = scaled(data[D_COOK], data[D_COOK_TOTAL], 24) as f32;
                sprite(draw, texture, o, 79.0, 34.0, 176.0, 14.0, i + 1.0, 16.0);
            }
            2 => {
                if data[D_GEN] > 0 {
                    let i = scaled(data[D_GEN], data[D_GEN_TOTAL].max(1), 13) as f32;
                    sprite(draw, GENERATOR, o, 56.0, 23.0 + 12.0 - i, 176.0, 12.0 - i, 14.0, i + 1.0);
                }
                let h = scaled(wide(data, D_ENERGY, D_ENERGY_HI), wide(data, D_CAPACITY, D_CAPACITY_HI), 42) as f32;
                sprite(draw, ENERGY, o, 109.0, 22.0 + 42.0 - h, 0.0, 42.0 - h, 14.0, h);
            }
            _ => {
                if tier > 0 {
                    sprite(draw, FACTORY, o, 48.0, 5.0, 176.0, 64.0, 18.0, 67.0);
                    sprite(draw, FACTORY, o, 111.0, 5.0, 176.0, 64.0, 18.0, 67.0);
                    if tier == 2 {
                        sprite(draw, FACTORY, o, 27.0, 5.0, 176.0, 64.0, 18.0, 67.0);
                        sprite(draw, FACTORY, o, 132.0, 5.0, 176.0, 64.0, 18.0, 67.0);
                    }
                }
                let h = scaled(wide(data, D_ENERGY, D_ENERGY_HI), wide(data, D_CAPACITY, D_CAPACITY_HI), 42) as f32;
                sprite(draw, ENERGY, o, 9.0, 7.0 + 42.0 - h, 0.0, 42.0 - h, 14.0, h);
                for j in 0..6 {
                    let i = scaled(data[D_FACTORY_COOK + j], data[D_FACTORY_TOTAL + j], 22) as f32;
                    sprite(draw, FACTORY, o, 29.0 + 21.0 * j as f32, 27.0, 176.0, 0.0, 15.0, i + 1.0);
                }
            }
        }
        if m == 1 {
            button(draw, o, R_SPLIT, mouse, Some([70.0, 132.0]), (data[D_SPLIT] != 0).then_some([70.0, 132.0]));
        }
    }
    // addInventoryButtons: the config tab beside the panel.
    if !config_open {
        sprite(draw, WIDGETS, o, -13.0, 2.0, 0.0, 0.0, 12.0, 13.0);
    } else {
        sprite(draw, WIDGETS, o, -56.0, 0.0, 0.0, 13.0, 60.0, 91.0);
        button(draw, o, R_AUTO_IN, mouse, Some([0.0, 132.0]), (data[D_AUTO_IN] != 0).then_some([0.0, 132.0]));
        button(draw, o, R_AUTO_OUT, mouse, Some([14.0, 132.0]), (data[D_AUTO_OUT] != 0).then_some([14.0, 132.0]));
        for (i, r) in R_SIDES.iter().enumerate() {
            let s = data[D_SIDES + i];
            if s != 0 {
                button(draw, o, *r, mouse, None, Some([(10 * s - 10) as f32, 104.0]));
            }
        }
        if !augment_page {
            let settings = &data[D_SIDES..D_SIDES + 6];
            let input = settings.iter().any(|s| matches!(*s, 1 | 3));
            let output = settings.iter().any(|s| matches!(*s, 2 | 3));
            let fuel = settings.contains(&4);
            let columns: &[f32] = match tier {
                0 => &[69.0, 90.0],
                1 => &[69.0, 90.0, 48.0, 111.0],
                _ => &[69.0, 90.0, 48.0, 111.0, 27.0, 132.0],
            };
            if input {
                if m == 0 {
                    sprite(draw, WIDGETS, o, 55.0, 16.0, 0.0, 114.0, 18.0, 18.0);
                }
                if m == 1 {
                    for x in columns {
                        sprite(draw, WIDGETS, o, *x, 5.0, 0.0, 114.0, 18.0, 18.0);
                    }
                }
            }
            if output {
                if m == 0 {
                    sprite(draw, WIDGETS, o, 111.0, 30.0, 0.0, 146.0, 26.0, 26.0);
                }
                if m == 1 {
                    for x in columns {
                        sprite(draw, WIDGETS, o, *x, 54.0, 36.0, 114.0, 18.0, 18.0);
                    }
                }
            }
            if fuel {
                if m == 0 {
                    sprite(draw, WIDGETS, o, 55.0, 52.0, 18.0, 114.0, 18.0, 18.0);
                }
                if m == 2 {
                    sprite(draw, WIDGETS, o, 55.0, 39.0, 18.0, 114.0, 18.0, 18.0);
                }
            }
        }
        // addRedstoneButtons.
        for (i, r) in R_RED.iter().enumerate() {
            let uv = [28.0 + 14.0 * i as f32, 132.0];
            button(draw, o, *r, mouse, Some(uv), (data[D_REDSTONE] == i as i32).then_some(uv));
        }
    }
    // extractLabels: the title (above the panel for a factory and the Rainbow Furnace), the inventory's label.
    let parts = title(host, block);
    let width: f32 = parts.iter().map(|(t, _)| draw.text_width(t)).sum();
    let above = m == 1 || data[D_RAINBOW] != 0;
    let x = if above {
        (88.0 - width / 2.0).floor()
    } else if m == 0 {
        (7.0 + 88.0 - width / 2.0).floor()
    } else {
        (88.0 - width / 2.0).floor()
    };
    let (y, colour) = if above { (-10.0, 0xFFFF_FFFF) } else { (6.0, 0xFF40_4040) };
    let mut cx = o[0] + x;
    for (text, c) in &parts {
        draw.text(text, [cx, o[1] + y], if *c == 0 { colour } else { *c }, false);
        cx += draw.text_width(text);
    }
    draw.text(&lang(host, "container.inventory", "Inventory"), [o[0] + 8.0, o[1] + 73.0], 0xFF40_4040, false);
}

fn draw_heater(host: &dyn ClientHost, data: &[i32], draw: &mut dyn DrawContext, o: [f32; 2], _mouse: [f32; 2]) {
    sprite(draw, "ironfurnaces:gui/heater", o, 0.0, 0.0, 0.0, 0.0, 176.0, 166.0);
    if data.len() >= 2 && data[1] > 0 {
        let i = scaled(data[0], data[1], 42) as f32;
        sprite(draw, "ironfurnaces:gui/heater", o, 67.0, 63.0, 176.0, 0.0, i + 1.0, 14.0);
    }
    draw.text(&lang(host, "container.inventory", "Inventory"), [o[0] + 8.0, o[1] + 72.0], 0xFF40_4040, false);
    let name = lang(host, "container.ironfurnaces.wireless_energy_heater", "Wireless Heater");
    draw.text(&name, [o[0] + 49.0, o[1] + 6.0], 0xFF40_4040, false);
}

fn shift(host: &dyn ClientHost) -> bool {
    host.is_key_down("key.keyboard.left.shift") || host.is_key_down("key.keyboard.right.shift")
}

/// SimpleSoundInstance.forUI(UI_BUTTON_CLICK, pitch, 0.3).
fn click_sound(host: &dyn ClientHost, pitch: f32) {
    let _ = host.play_local_sound(&SoundPlay::ui("minecraft:ui.button.click").with_pitch(pitch).with_volume(0.3), 0);
}

/// mouseClicked: the button the click presses, if any; the config tab's opening and closing stay on the client
/// (ClientShowConfig).
pub(crate) fn click(host: &dyn ClientHost, block: &str, data: &[i32], at: [f32; 2], right: bool) -> Option<i32> {
    if !is_furnace(block) || data.len() < DATA_SLOTS {
        return None;
    }
    let button = if right { 1 } else { 0 };
    let config_open = with_state(|s| s.config_open);
    let mut pressed = None;
    if config_open {
        // mouseClickedRedstoneButtons.
        for (i, r) in R_RED.iter().enumerate() {
            if button == 0 && data[D_REDSTONE] != i as i32 && inside(*r, at) {
                click_sound(host, 0.6);
                pressed = Some(B_REDSTONE + i as i32);
            }
        }
    }
    // mouseClickedInventoryButtons.
    if !config_open {
        if at[0] >= -17.0 && at[0] <= 1.0 && at[1] >= -1.0 && at[1] <= 17.0 {
            with_state(|s| s.config_open = true);
        }
    } else {
        if at[0] >= -13.0 && at[0] <= 0.0 && at[1] >= 5.0 && at[1] <= 18.0 {
            with_state(|s| s.config_open = false);
        }
        if button == 0 && inside(R_AUTO_IN, at) {
            click_sound(host, 0.6);
            pressed = Some(B_AUTO_IN);
        }
        if button == 0 && inside(R_AUTO_OUT, at) {
            click_sound(host, 0.6);
            pressed = Some(B_AUTO_OUT);
        }
        for (i, r) in R_SIDES.iter().enumerate() {
            if !inside(*r, at) {
                continue;
            }
            if i == 2 && shift(host) {
                click_sound(host, 0.6);
                pressed = Some(B_RESET_SIDES);
                continue;
            }
            let setting = data[D_SIDES + i];
            let next = match button {
                0 => Some(if setting == 4 { 0 } else { setting + 1 }),
                1 => Some(if setting == 0 { 4 } else { setting - 1 }),
                _ => None,
            };
            if let Some(next) = next {
                click_sound(host, if button == 0 { 0.6 } else { 0.3 });
                pressed = Some(B_SIDE + i as i32 * 5 + next);
            }
        }
    }
    // mouseClickedAugmentButton.
    if button == 0 && inside(R_AUGMENT, at) {
        click_sound(host, 0.6);
        pressed = Some(B_AUGMENT_GUI);
    }
    // mouseClickedAutoSplitButton.
    if button == 0 && data[D_MODE] == 1 && inside(R_SPLIT, at) {
        click_sound(host, 0.6);
        pressed = Some(B_SPLIT);
    }
    pressed
}

fn side_words(host: &dyn ClientHost, setting: i32) -> String {
    match setting {
        1 => lang(host, "tooltip.ironfurnaces.gui_input", "Input"),
        2 => lang(host, "tooltip.ironfurnaces.gui_output", "Output"),
        3 => lang(host, "tooltip.ironfurnaces.gui_input_output", "Input/Output"),
        4 => lang(host, "tooltip.ironfurnaces.gui_fuel", "Fuel Input/Eject"),
        _ => lang(host, "tooltip.ironfurnaces.gui_none", "NONE"),
    }
}

/// displayEnergy: "1,234 / 80,000 RF".
pub(crate) fn energy_text(energy: i32, capacity: i32) -> String {
    format!("{} / {} RF", grouped(energy), grouped(capacity))
}

pub(crate) fn grouped(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// addTooltips: the lines under the pointer (also where the hover highlights are drawn from).
pub(crate) fn tooltip(host: &dyn ClientHost, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    with_state(|s| s.mouse = at);
    if is_heater(block) {
        if data.len() >= 2 && at[0] >= 68.0 && at[0] <= 108.0 && at[1] >= 64.0 && at[1] <= 76.0 {
            lines.push(energy_text(data[0], data[1]));
        }
        return;
    }
    if !is_furnace(block) || data.len() < DATA_SLOTS {
        return;
    }
    let config_open = with_state(|s| s.config_open);
    let augment_page = data[D_AUGMENT_GUI] != 0;
    let m = data[D_MODE];
    if inside(R_AUGMENT, at) {
        lines.push(if augment_page {
            lang(host, "tooltip.ironfurnaces.gui_open_furnace", "Furnace")
        } else {
            lang(host, "tooltip.ironfurnaces.gui_open_augments", "Augments")
        });
    }
    let bar = match (augment_page, m) {
        (false, 1) => Some([9.0, 7.0, 14.0, 42.0]),
        (false, 2) => Some([109.0, 22.0, 14.0, 42.0]),
        _ => None,
    };
    if let Some(bar) = bar
        && inside(bar, at)
    {
        lines.push(energy_text(wide(data, D_ENERGY, D_ENERGY_HI), wide(data, D_CAPACITY, D_CAPACITY_HI)));
    }
    if m == 1 && !augment_page && inside(R_SPLIT, at) {
        lines.push("Auto Split".to_owned());
        lines.push(if data[D_SPLIT] != 0 { "ON" } else { "OFF" }.to_owned());
    }
    if !config_open {
        if at[0] >= -16.0 && at[0] <= 0.0 && at[1] >= 0.0 && at[1] <= 16.0 {
            lines.push(lang(host, "tooltip.ironfurnaces.gui_open", "Open Config"));
        }
        return;
    }
    if at[0] >= -12.0 && at[0] <= -1.0 && at[1] >= 6.0 && at[1] <= 17.0 {
        lines.push(lang(host, "tooltip.ironfurnaces.gui_close", "Hide Config"));
    }
    let on = |v: i32| if v != 0 { "ON" } else { "OFF" }.to_owned();
    if inside(R_AUTO_IN, at) {
        lines.push(lang(host, "tooltip.ironfurnaces.gui_auto_input", "Auto Input"));
        lines.push(on(data[D_AUTO_IN]));
    }
    if inside(R_AUTO_OUT, at) {
        lines.push(lang(host, "tooltip.ironfurnaces.gui_auto_output", "Auto Output"));
        lines.push(on(data[D_AUTO_OUT]));
    }
    const NAMES: [(&str, &str); 6] =
        [("gui_bottom", "Bottom"), ("gui_top", "Top"), ("gui_front", "Front"), ("gui_back", "Back"), ("gui_left", "Left"), ("gui_right", "Right")];
    for (i, r) in R_SIDES.iter().enumerate() {
        if !inside(*r, at) {
            continue;
        }
        if i == 2 && shift(host) {
            lines.push(lang(host, "tooltip.ironfurnaces.gui_reset", "Reset All"));
        } else {
            lines.push(lang(host, &format!("tooltip.ironfurnaces.{}", NAMES[i].0), NAMES[i].1));
            lines.push(side_words(host, data[D_SIDES + i]));
        }
    }
    const RED: [(&str, &str); 3] =
        [("gui_redstone_ignored", "Redstone Ignored"), ("gui_redstone_low", "Redstone Low"), ("gui_redstone_high", "Redstone High")];
    for (i, r) in R_RED.iter().enumerate() {
        if inside(*r, at) {
            lines.push(lang(host, &format!("tooltip.ironfurnaces.{}", RED[i].0), RED[i].1));
        }
    }
}

/// The server's half of a button: PacketSplitFurnaceSetting, PacketAugmentGUIFurnaceSetting, PacketAutoIOFurnaceSetting,
/// PacketRedstoneFurnaceSetting and PacketFurnaceSettings. Only the button ids the screen sends are taken.
pub(crate) fn menu_button(host: &mut dyn ServerHost, player: u128, pos: Pos, id: i32, nbt: &mut Compound) {
    let Some(level) = host.player(player).and_then(|p| p.level(host)) else { return };
    let Some(block) = level.block(host, pos).and_then(|b| b.id(host).map(str::to_owned)) else { return };
    if !is_furnace(&block) {
        return;
    }
    let toggle = |nbt: &mut Compound, key: &str| {
        let v = get_i(nbt, key, 0);
        set_i(nbt, key, i32::from(v == 0));
    };
    match id {
        B_SPLIT => toggle(nbt, "AutoSplit"),
        B_AUGMENT_GUI => {
            toggle(nbt, "AugmentGUI");
            host.block_entity_sync(pos);
        }
        B_AUTO_IN | B_AUTO_OUT => {
            let mut auto = int_array(nbt, "AutoIO", 2);
            let i = (id - B_AUTO_IN) as usize;
            auto[i] = i32::from(auto[i] == 0);
            set_int_array(nbt, "AutoIO", auto);
        }
        4..=6 => set_i(nbt, "Redstone", id - B_REDSTONE),
        B_RESET_SIDES => set_int_array(nbt, "Settings", vec![0; 6]),
        _ if (B_SIDE..B_SIDE + 30).contains(&id) => {
            let (i, setting) = (((id - B_SIDE) / 5) as usize, (id - B_SIDE) % 5);
            let facing = level.block_property(host, pos, "facing").unwrap_or_else(|| "north".to_owned());
            let order = furnace::screen_sides(&facing);
            let mut sides = int_array(nbt, "Settings", 6);
            sides[order[i]] = setting;
            set_int_array(nbt, "Settings", sides);
        }
        _ => {}
    }
}
