//! Right clicks: BlockIronFurnaceBase.useWithoutItem (augments, the spooky and wrapping looks, the Settings Linker's copy,
//! opening the screen), ItemUpgrade.useOn and ItemFurnaceCopy.useOn (sneaking), the Wireless Heat Transmitter's
//! screen, and getStateForPlacement. Server only: the server decides, from what the player really holds.

use crate::furnace::{self, SLOTS};
use crate::util::*;
use mod_api::*;

fn player_of(host: &dyn Host, uuid: u128) -> Option<(PlayerRef, LevelRef)> {
    let player = host.player(uuid)?;
    let level = player.level(host)?;
    Some((player, level))
}

/// The inventory slot of what the player holds in `hand`: as a slot, so a write changes that stack itself (a creative
/// player's hand, as a place, would keep it and give a copy).
fn hand_slot(host: &dyn Host, player: u128, hand: Hand) -> ItemPlace {
    let slot = match hand {
        Hand::Off => -106,
        _ => host.player(player).and_then(|p| p.selected_slot(host)).unwrap_or(0) as i32,
    };
    ItemPlace::PlayerSlot { player, slot }
}

/// Takes one of what the player holds in `hand`; the caller leaves a creative player's (`!player.isCreative()`).
fn take_one(host: &mut dyn ServerHost, player: u128, hand: Hand, stack: &ItemStack) {
    let place = hand_slot(&*host, player, hand);
    let resource = ItemResource::of(stack);
    let _ = host.transaction(|host, tx| match place.extract(host, tx, &resource, 1) {
        Ok(1) => Ok(()),
        _ => Err(()),
    });
}

/// The facing a block placed along `direction` by a non-player (DirectionalPlaceContext) gets: the opposite of its
/// horizontal direction, which is the direction itself, or north when it is vertical.
pub(crate) fn facing_for_direction(direction: Direction) -> &'static str {
    match direction {
        Direction::South => "north",
        Direction::West => "east",
        Direction::East => "west",
        Direction::North | Direction::Up | Direction::Down => "south",
    }
}

/// The facing a block placed by a player looking `yaw` gets: their horizontal direction's opposite.
pub(crate) fn facing_for(yaw: f32) -> &'static str {
    // Direction.fromYRot: south, west, north, east; then getOpposite.
    ["north", "east", "south", "west"][((yaw / 90.0 + 0.5).floor() as i32 & 3) as usize]
}

/// Opens the furnace's screen for `player`: the page shown is the furnace's own (AugmentGUI off, as interactWith).
pub(crate) fn open(host: &mut dyn ServerHost, player: PlayerRef, level: LevelRef, pos: Pos) -> bool {
    let Some(entity) = host.block_entity(level, pos) else { return false };
    if entity.data(host).is_some_and(|d| get_i(&d, "AugmentGUI", 0) != 0) {
        let mut c = Compound::new();
        set_i(&mut c, "AugmentGUI", 0);
        let _ = entity.merge_data(host, c);
        host.block_entity_sync(pos);
    }
    matches!(player.open_menu(host, entity), Ok(true))
}

/// BlockIronFurnaceBase.useWithoutItem with an item in hand, not sneaking.
pub(crate) fn use_item_on(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let (player, level) = player_of(host, click.player)?;
    let stack = click.stack?;
    if sneaking(host, player) {
        return None;
    }
    let pos = click.pos;
    let id = id_of(host, stack);
    let entity = host.block_entity(level, pos)?;
    if let Some(slot) = furnace::augment_slot(&id) {
        // interactAugment: the old one comes out (not for a creative player), the new one goes in.
        let creative = player.is_creative(host).unwrap_or(false);
        let old = entity.item_slot(host, slot as u32).ok().flatten();
        let one = with_count(stack, 1)?;
        let placed = host
            .transaction(|host, tx| match entity.set_item_slot(host, tx, slot as u32, Some(one.clone())) {
                Ok(true) => Ok(()),
                _ => Err(()),
            })
            .is_ok();
        if !placed {
            return Some(InteractionResult::Success);
        }
        if let Some(old) = old
            && !creative
            && let Some(old) = with_count(&old, 1)
        {
            let _ = level.spawn_item(host, f64::from(pos[0]), f64::from(pos[1] + 1), f64::from(pos[2]), old, 10);
        }
        let _ = level.play_sound(host, "minecraft:block.anvil.use", centre(pos), SoundSource::Blocks, 0.05, 1.0, None);
        if !creative {
            take_one(host, click.player, click.hand, stack);
        }
        host.block_entity_sync(pos);
        return Some(InteractionResult::Success);
    }
    match id.as_str() {
        "ironfurnaces:item_spooky" => set_jovial(host, entity, 1),
        "ironfurnaces:item_xmas" => set_jovial(host, entity, 2),
        "ironfurnaces:item_copy" => copy_settings(host, player, click, level, pos),
        _ => return None,
    }
    Some(InteractionResult::Success)
}

/// BlockIronFurnaceBase.useWithoutItem with an empty hand: sneaking unwraps (jovial 0), else the screen opens.
pub(crate) fn use_without_item(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let (player, level) = player_of(host, click.player)?;
    let pos = click.pos;
    if sneaking(host, player) {
        // A sneaking click with an item is the item's (an upgrade's, the linker's useOn), not the block's.
        let held = click.stack.cloned().or_else(|| ItemPlace::PlayerHand { player: click.player, hand: click.hand }.with_stack(host, |s| s.project()).flatten());
        if held.is_some() {
            return None;
        }
        let entity = host.block_entity(level, pos)?;
        set_jovial(host, entity, 0);
        return Some(InteractionResult::Success);
    }
    open(host, player, level, pos).then_some(InteractionResult::SuccessServer)
}

fn set_jovial(host: &mut dyn ServerHost, entity: BlockEntityRef, jovial: i32) {
    let mut c = Compound::new();
    set_i(&mut c, "Jovial", jovial);
    let _ = entity.merge_data(host, c);
    if let Some(pos) = entity.position(host) {
        host.set_block_property_with(pos, "jovial", &jovial.to_string(), 3);
    }
}

/// interactCopy: the furnace's settings onto the Settings Linker in hand (FurnaceSettings.writeToTag and `direction`).
fn copy_settings(host: &mut dyn ServerHost, player: PlayerRef, click: &BlockClick<'_>, level: LevelRef, pos: Pos) {
    let Some(stack) = click.stack else { return };
    let Some(data) = host.block_entity(level, pos).and_then(|e| e.data(host)) else { return };
    let mut tag = Compound::new();
    set_int_array(&mut tag, "Settings", int_array(&data, "Settings", 6));
    set_int_array(&mut tag, "AutoIO", int_array(&data, "AutoIO", 2));
    set_i(&mut tag, "Redstone", get_i(&data, "Redstone", 0));
    set_i(&mut tag, "AugmentGUI", get_i(&data, "AugmentGUI", 0));
    set_i(&mut tag, "AutoSplit", get_i(&data, "AutoSplit", 0));
    let facing = level.block_property(host, pos, "facing").unwrap_or_else(|| "north".to_owned());
    let direction = ["down", "up", "north", "south", "west", "east"].iter().position(|f| *f == facing).unwrap_or(2) as i32;
    set_i(&mut tag, "direction", direction);
    let mut written = stack.clone();
    let mut custom = custom_data(&written).unwrap_or_default();
    custom.insert(java("FurnaceSettings"), Tag::Compound(tag));
    set_custom_data(&mut written, custom);
    let place = hand_slot(&*host, click.player, click.hand);
    let resource = ItemResource::of(&written);
    let _ = host.transaction(|host, tx| match place.exchange(host, tx, &resource, stack.count()) {
        Ok(0) | Err(_) => Err(()),
        Ok(_) => Ok(()),
    });
    let _ = player.tell(host, Text::literal("Settings copied"));
}

/// The settings a Settings Linker holds.
pub(crate) fn linker_settings(stack: &ItemStack) -> Option<Compound> {
    match custom_data(stack)?.get(&java("FurnaceSettings")) {
        Some(Tag::Compound(c)) if !c.is_empty() => Some(c.clone()),
        _ => None,
    }
}

/// ItemFurnaceCopy.useOn, sneaking: the held settings onto the furnace, and its facing.
fn apply_settings(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack, level: LevelRef, pos: Pos) -> bool {
    let block = level.block(host, pos).and_then(|b| b.id(host).map(str::to_owned)).unwrap_or_default();
    if !furnace::is_furnace(&block) {
        return false;
    }
    let Some(entity) = host.block_entity(level, pos) else { return false };
    if let Some(tag) = linker_settings(stack) {
        let mut c = Compound::new();
        for key in ["Settings", "AutoIO", "Redstone", "AugmentGUI", "AutoSplit"] {
            if let Some(v) = tag.get(&java(key)) {
                c.insert(java(key), v.clone());
            }
        }
        let _ = entity.merge_data(host, c);
        let facing = ["down", "up", "north", "south", "west", "east"].get(get_i(&tag, "direction", 0).clamp(0, 5) as usize).copied();
        if let Some(facing) = facing
            && facing != "up"
            && facing != "down"
            && level.block_property(host, pos, "facing").as_deref() != Some(facing)
        {
            host.set_block_property_with(pos, "facing", facing, 3);
        }
    }
    let _ = player.tell(host, Text::literal("Settings applied"));
    true
}

/// ItemUpgrade: (from, to).
pub(crate) fn upgrade_of(item: &str) -> Option<(&'static str, &'static str)> {
    Some(match item {
        "ironfurnaces:upgrade_iron" => ("minecraft:furnace", "ironfurnaces:iron_furnace"),
        "ironfurnaces:upgrade_gold" => ("ironfurnaces:iron_furnace", "ironfurnaces:gold_furnace"),
        "ironfurnaces:upgrade_diamond" => ("ironfurnaces:gold_furnace", "ironfurnaces:diamond_furnace"),
        "ironfurnaces:upgrade_emerald" => ("ironfurnaces:diamond_furnace", "ironfurnaces:emerald_furnace"),
        "ironfurnaces:upgrade_obsidian" => ("ironfurnaces:emerald_furnace", "ironfurnaces:obsidian_furnace"),
        "ironfurnaces:upgrade_crystal" => ("ironfurnaces:diamond_furnace", "ironfurnaces:crystal_furnace"),
        "ironfurnaces:upgrade_obsidian2" => ("ironfurnaces:crystal_furnace", "ironfurnaces:obsidian_furnace"),
        "ironfurnaces:upgrade_netherite" => ("ironfurnaces:obsidian_furnace", "ironfurnaces:netherite_furnace"),
        "ironfurnaces:upgrade_copper" => ("minecraft:furnace", "ironfurnaces:copper_furnace"),
        "ironfurnaces:upgrade_silver" => ("ironfurnaces:copper_furnace", "ironfurnaces:silver_furnace"),
        "ironfurnaces:upgrade_silver2" => ("ironfurnaces:iron_furnace", "ironfurnaces:silver_furnace"),
        "ironfurnaces:upgrade_iron2" => ("ironfurnaces:copper_furnace", "ironfurnaces:iron_furnace"),
        "ironfurnaces:upgrade_gold2" => ("ironfurnaces:silver_furnace", "ironfurnaces:gold_furnace"),
        "ironfurnaces:upgrade_allthemodium" => ("ironfurnaces:netherite_furnace", "ironfurnaces:allthemodium_furnace"),
        "ironfurnaces:upgrade_vibranium" => ("ironfurnaces:allthemodium_furnace", "ironfurnaces:vibranium_furnace"),
        "ironfurnaces:upgrade_unobtainium" => ("ironfurnaces:vibranium_furnace", "ironfurnaces:unobtainium_furnace"),
        _ => return None,
    })
}

/// ItemUpgrade.useOn: the `from` block becomes `to`, placed as the player would place it (facing them). A vanilla
/// furnace's three slots drop; an Iron Furnace's contents, energy, progress and settings go over, as the Java mod
/// copies them field by field (its augment record, recipes kept for experience and owner don't).
fn upgrade(host: &mut dyn ServerHost, player: PlayerRef, uuid: u128, hand: Hand, stack: &ItemStack, level: LevelRef, pos: Pos) -> bool {
    let item = id_of(host, stack);
    let Some((from, to)) = upgrade_of(&item) else { return false };
    let Some(block) = level.block(host, pos).and_then(|b| b.id(host).map(str::to_owned)) else { return false };
    if block != from {
        return false;
    }
    let Some(old) = host.block_entity(level, pos) else { return false };
    let yaw = player.entity().rotation(host).map_or(0.0, |r| r[0]);
    let Some(next) = host.block_state_id(&format!("{to}[facing={}]", facing_for(yaw))) else { return false };
    let mut kept = None;
    if from == "minecraft:furnace" {
        for i in 0..3 {
            if let Ok(Some(s)) = old.item_slot(host, i) {
                let _ = level.spawn_item(host, f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]), s, 10);
            }
        }
    } else {
        let mut nbt = old.data(host).unwrap_or_default();
        let stale: Vec<JavaString> = nbt
            .keys()
            .filter(|k| {
                let k = k.to_string_lossy();
                k.starts_with("RecipesUsed.") || k == "Augment" || k == "Owner"
            })
            .cloned()
            .collect();
        for k in stale {
            nbt.remove(&k);
        }
        let items: Vec<Option<ItemStack>> = (0..SLOTS as u32).map(|i| old.item_slot(host, i).ok().flatten()).collect();
        kept = Some((nbt, items));
    }
    let Some(air) = host.block_state_id("minecraft:air") else { return false };
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    // (Its slots are emptied first, so the old block entity's removal drops nothing.)
    if kept.is_some() {
        let _ = host.transaction(|host, tx| {
            for i in 0..SLOTS as u32 {
                let _ = old.set_item_slot(host, tx, i, None);
            }
            Ok::<(), ()>(())
        });
    } else {
        let _ = host.transaction(|host, tx| {
            for i in 0..3 {
                let _ = old.set_item_slot(host, tx, i, None);
            }
            Ok::<(), ()>(())
        });
    }
    let _ = level.set_block(host, pos, air, flags);
    let _ = level.set_block(host, pos, next, flags);
    if let Some((nbt, items)) = kept
        && let Some(entity) = host.block_entity(level, pos)
    {
        let _ = entity.replace_data(host, nbt);
        let _ = host.transaction(|host, tx| {
            for (i, s) in items.into_iter().enumerate() {
                let _ = entity.set_item_slot(host, tx, i as u32, s);
            }
            Ok::<(), ()>(())
        });
    }
    if !player.is_creative(host).unwrap_or(false) {
        take_one(host, uuid, hand, stack);
    }
    true
}

/// ItemPart::use_on for the upgrades and the Settings Linker (a sneaking click reaches the item, not the block).
pub(crate) fn item_use_on(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Flow {
    let Some((player, level)) = player_of(host, event.player) else { return Flow::Continue };
    let id = id_of(host, event.stack);
    if upgrade_of(&id).is_some() {
        return if upgrade(host, player, event.player, event.hand, event.stack, level, event.pos) { Flow::Cancel } else { Flow::Continue };
    }
    // (A click that isn't sneaking on a furnace is the block's: the copy, not this.)
    if id == "ironfurnaces:item_copy" && apply_settings(host, player, event.stack, level, event.pos) {
        return Flow::Cancel;
    }
    Flow::Continue
}

/// The Wireless Heat Transmitter's screen.
pub(crate) fn heater_use(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let (player, level) = player_of(host, click.player)?;
    if sneaking(host, player) && click.stack.is_some() {
        return None;
    }
    let entity = host.block_entity(level, click.pos)?;
    matches!(player.open_menu(host, entity), Ok(true)).then_some(InteractionResult::SuccessServer)
}
