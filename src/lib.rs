//! Iron Furnaces (Qelifern, XenoMustache, Astryxion; Apache-2.0) for Sunrise: thirteen
//! furnaces with upgrades and augments, factories and generators, side settings, auto input and output, redstone
//! modes, the Wireless Heat Transmitter and Receiver, the Settings Linker, the Rainbow Furnace, and the W guide.
//! A port of BlockIronFurnaceTileBase, BlockIronFurnaceBase, the screens, containers and items of the 4.4.2 jar, written
//! against Hooks and Parts only (no mixins, no `[[inject]]`), so it builds against the Sunrise SDK.
//!
//! Where things run:
//! - the server ticks each furnace (`furnace::tick`, lent the block entity) and answers its item and energy
//!   capabilities (`caps`), so hoppers, pipes and other mods' machines and cables reach it as the Java mod's;
//! - right clicks, upgrades and the Settings Linker are the server's (`interact`), from what the player really holds;
//! - the screens are drawn and clicked on the client (`screen`); the server acts on the button ids (`menu_button`)
//!   and checks every slot through the menu (shown slots, slot rules, quick moves).

mod commands_menu;
mod ae2_eta;
mod work;
mod caps;
mod client;
mod config;
mod furnace;
mod interact;
mod jei;
mod owners;
mod quickmove;
mod recipes;
mod screen;
mod tooltip;
mod util;

use crate::furnace::{FURNACES, is_furnace, is_heater, is_rainbow};
use crate::util::*;
use mod_api::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

/// What both sides keep between hooks. One mod instance serves the client and the server game, so it is behind a
/// lock (the fork's rule: hooks are `&self + Send + Sync`).
#[derive(Default)]
pub(crate) struct Shared {
    /// Server: who placed a furnace, until its block entity's first tick (setPlacedBy), by dimension and position.
    pub placed: HashMap<(String, Pos), u128>,
    /// Server: what a furnace kept for experience, and a transmitter's energy, from its removal to its drops.
    pub xp_on_break: HashMap<Pos, u32>,
    pub heater_on_break: HashMap<Pos, i32>,
    /// Server: Rainbow Furnaces a blast took apart (onExplosionHit), until their drops.
    pub exploded: HashSet<Pos>,
    /// Server: each furnace's comparator output, by dimension and position.
    pub comparator: HashMap<(String, Pos), i32>,
    /// Server: the energy a transmitter item being placed holds, by player.
    pub placing_heater: HashMap<u128, i32>,
    /// Client: the pointer over the open screen, from its origin.
    pub mouse: [f32; 2],
    /// Client: ClientShowConfig, the config tab.
    pub config_open: bool,
    /// Client: the Rainbow Furnace's title colours.
    pub title_timer: u64,
    pub title_seed: u64,
    pub client_ticks: u64,
}

pub(crate) static STATE: Mutex<Option<Shared>> = Mutex::new(None);

pub(crate) fn with_state<T>(f: impl FnOnce(&mut Shared) -> T) -> T {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Shared::default))
}

/// Keys looked up at registration.
pub(crate) struct Keys {
    pub bucket: ItemKey,
    pub item_heater: ItemKey,
    pub heater: ItemKey,
    pub million: ItemKey,
    pub rainbow_coal: ItemKey,
    pub augments: [(ItemKey, usize); 6],
    pub smoke: ParticleKey,
    pub flame: ParticleKey,
    pub crit: ParticleKey,
}

pub(crate) static KEYS: OnceLock<Keys> = OnceLock::new();

#[derive(Default)]
struct Mod;

/// The furnaces' Part: their blocks and block entities.
pub(crate) struct Furnace;
/// The Wireless Heat Transmitter's Part.
pub(crate) struct Heater;
/// The upgrades' and the Settings Linker's Part (their useOn).
pub(crate) struct Tool;
/// The transmitter's item: its stored energy's bar, and the energy it places with.
pub(crate) struct HeaterItem;
/// The menus' slot rules.
pub(crate) struct Menus;

const UPGRADES: [&str; 16] = [
    "upgrade_iron", "upgrade_gold", "upgrade_diamond", "upgrade_emerald", "upgrade_obsidian", "upgrade_crystal",
    "upgrade_obsidian2", "upgrade_netherite", "upgrade_copper", "upgrade_silver", "upgrade_silver2", "upgrade_iron2",
    "upgrade_gold2", "upgrade_allthemodium", "upgrade_vibranium", "upgrade_unobtainium",
];

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    /// `/<ns>`: the commands menu (`commands_menu.rs`), answered on the server with the player's level.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(commands_menu::command());
    }

    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        // `/<ns>` commands menu: the client's side of its channel.
        commands_menu::register_payload(r);
        // The generators' recipes, read with `recipes_of_type`: Sunrise refuses a recipe file whose serializer no mod
        // registered, so the type and its serializer (the files' `type`, the same id) are declared here.
        r.register_recipe_type("ironfurnaces:generator_blasting");
        r.register_raw_recipe_serializer("ironfurnaces:generator_blasting", "ironfurnaces:generator_blasting");
        let furnaces = r.block_entity_type("furnace").blocks(&FURNACES).build();
        r.block_entity_part(&furnaces, Furnace).expect("ironfurnaces:furnace is a native block entity type");
        let heater = r.block_entity_type("heater").blocks(&["ironfurnaces:heater"]).build();
        r.block_entity_part(&heater, Heater).expect("ironfurnaces:heater is a native block entity type");
        r.block_entity_capability_before_game(&ITEMS, &furnaces, |_, at, _, _, offer| {
            offer.offer(|| Box::new(caps::FurnaceItems(at)))
        });
        r.block_entity_capability_before_game(&ENERGY, &furnaces, |_, at, _, _, offer| {
            offer.offer(|| Box::new(caps::FurnaceEnergy(at)))
        });
        // With the Machine Work library installed: the furnaces' progress and craft times through its shared capability
        // (AE2's crafting ETA, Jade, recipe viewers read it), `work.rs`.
        #[cfg(feature = "machine_work")]
        if _cx.mods().get("machine_work").is_some() {
            r.block_entity_capability(&machine_work::MACHINE_WORK, &furnaces, |_, at, _, _, offer| {
                offer.offer(|| Box::new(work::FurnaceWork(at)) as Box<dyn machine_work::MachineWork>)
            });
        }
        r.menu_part("ironfurnaces:furnace", Menus);
        r.menu_part("ironfurnaces:heater", Menus);
        // The screens: built on the client for each opening, from the block it opened on.
        for menu in ["ironfurnaces:furnace", "ironfurnaces:heater"] {
            let _ = r.register_menu_screen(menu, std::sync::Arc::new(screen::create));
        }
        for name in UPGRADES.iter().chain(&["item_copy"]) {
            let key = r.item_key(&format!("ironfurnaces:{name}"));
            r.item_part(key, Tool);
        }
        let heater_item = r.item_key("ironfurnaces:heater");
        r.item_part(heater_item, HeaterItem);
        let mut augment = |id: &str, slot| (r.item_key(id), slot);
        let augments = [
            augment("ironfurnaces:augment_blasting", furnace::AUGMENT_RED),
            augment("ironfurnaces:augment_smoking", furnace::AUGMENT_RED),
            augment("ironfurnaces:augment_speed", furnace::AUGMENT_GREEN),
            augment("ironfurnaces:augment_fuel", furnace::AUGMENT_GREEN),
            augment("ironfurnaces:augment_factory", furnace::AUGMENT_BLUE),
            augment("ironfurnaces:augment_generator", furnace::AUGMENT_BLUE),
        ];
        let keys = Keys {
            bucket: r.item_key("minecraft:bucket"),
            item_heater: r.item_key("ironfurnaces:item_heater"),
            heater: heater_item,
            million: r.item_key("ironfurnaces:million_furnace"),
            rainbow_coal: r.item_key("ironfurnaces:rainbow_coal"),
            augments,
            smoke: r.particle_key("minecraft:smoke"),
            flame: r.particle_key("minecraft:flame"),
            crit: r.particle_key("minecraft:crit"),
        };
        let _ = KEYS.set(keys);

        // The game's own answer for the slots; hoppers and pipes go through `caps::FurnaceItems`, which answers
        // first with the furnace's side settings.
        let all: Vec<usize> = (0..furnace::SLOTS).collect();
        for id in FURNACES {
            let block = r.block_key(id);
            r.block_container(
                &block,
                ModContainer {
                    slots: furnace::SLOTS,
                    faces: std::array::from_fn(|_| all.clone()),
                    inputs: vec![0, 1, 6, 7, 8, 9, 10, 11, 12],
                    outputs: vec![1, 2, 6, 13, 14, 15, 16, 17, 18],
                    insert_rules: Vec::new(),
                    menu: Some("ironfurnaces:furnace".to_owned()),
                },
            );
        }
        let heater_block = r.block_key("ironfurnaces:heater");
        r.block_container(
            &heater_block,
            ModContainer {
                slots: 1,
                faces: std::array::from_fn(|_| Vec::new()),
                inputs: Vec::new(),
                outputs: Vec::new(),
                insert_rules: Vec::new(),
                menu: Some("ironfurnaces:heater".to_owned()),
            },
        );
        // The transmitter's FEnergyStorage(1000000, 1000000, 0): it takes power from any side and gives none. (The
        // furnaces' energy is `caps::FurnaceEnergy`, which follows their mode.)
        r.block_energy(
            &heater_block,
            ModEnergyBuffer {
                capacity: 1_000_000,
                max_insert: 1_000_000,
                max_extract: 0,
                insert_faces: [true; 6],
                extract_faces: [false; 6],
            },
        );
    }


    /// With AE2 installed: the furnaces' craft times for its crafting jobs' time left (`ae2_eta`).
    fn server_starting(&self, host: &mut dyn ServerHost, _event: &mut ServerStartingEvent) {
        mod_api::if_installed!(host, ae2, {
            ae2::api::register_machine(crate::ae2_eta::machine_time);
        });
    }

    fn tags_updated(&self, _context: &dyn ReloadContext, _event: &mut TagsUpdatedEvent) {
        recipes::forget();
    }

    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        let Some(placer) = event.placer.and_then(|e| e.uuid(host)) else { return Flow::Continue };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Some(block) = host.block_state_string(event.state) else { return Flow::Continue };
        let block = block.split('[').next().unwrap_or("").to_owned();
        if is_furnace(&block) {
            let dimension = host.dimension().to_owned();
            with_state(|s| s.placed.insert((dimension, pos), placer));
        } else if is_heater(&block) {
            // setPlacedBy: the energy the item held.
            let energy = with_state(|s| s.placing_heater.remove(&placer)).unwrap_or(0);
            if energy > 0
                && let Some(level) = host.level(host.dimension())
                && let Some(entity) = host.block_entity(level, pos)
            {
                let mut c = Compound::new();
                set_i(&mut c, "energy", energy.min(1_000_000));
                let _ = entity.merge_data(host, c);
            }
        }
        Flow::Continue
    }

    fn block_drops(&self, level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Some(block) = level.state_block(event.state).map(str::to_owned) else { return Flow::Continue };
        let Some(keys) = KEYS.get() else { return Flow::Continue };
        if is_furnace(&block) {
            // onBlockStateChange: grantStoredRecipeExperience where it stood.
            let xp = with_state(|s| s.xp_on_break.remove(&pos)).unwrap_or(0);
            event.experience += xp;
            if is_rainbow(&block) && with_state(|s| s.exploded.remove(&pos)) {
                // onExplosionHit: the block itself is gone, not dropped.
                event.drops.retain(|d| d.stack().is_none_or(|s| s.item() != keys.million));
            }
        } else if is_heater(&block) {
            let energy = with_state(|s| s.heater_on_break.remove(&pos)).unwrap_or(0);
            // onDestroyedByPlayer: a transmitter broken by a player (not in creative: no drops then) comes back with
            // its energy. Its loot table drops nothing.
            if event.breaker.is_some()
                && let Some(mut stack) = ItemStack::new(keys.heater, 1)
            {
                if energy > 0 {
                    let mut c = Compound::new();
                    set_i(&mut c, "Energy", energy);
                    set_custom_data(&mut stack, c);
                }
                let _ = event.drops.push(ItemDrop::new(stack, centre(pos), [0.0; 3], 10));
            }
        }
        Flow::Continue
    }

    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        // The stack and lines are lent as views: the lines are appended after the borrowed list.
        let Some(stack) = event.stack.project() else { return };
        let mut added = Vec::new();
        tooltip::tooltip(host, &stack, &mut added);
        for (text, color) in added {
            let _ = event.lines.push(text, color);
        }
    }

    fn client_player_logging_in(&self, host: &mut dyn ClientHost, _event: &mut ClientPlayerLoggingInEvent) {
        mod_api::if_installed!(host, jei, {
            crate::jei::register(&*host);
        });
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        client::tick(host);
    }
}

#[mod_api::sunrise_part]
impl BlockPart for Furnace {
    /// `misc.lightupdates` (the Java mod's light_state mixin): when set, a lit furnace gives no light, so lighting
    /// isn't recalculated each time one lights or goes out. Otherwise its `state_light` (14 lit) stands.
    fn light_emission(&self, host: &dyn Host, _event: &mut LightEmissionEvent) -> Option<u8> {
        (config::cfg(host, "misc.lightupdates") != 0).then_some(0)
    }

    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        interact::use_item_on(host, event.click)
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        interact::use_without_item(host, event.click)
    }

    fn state_for_placement(&self, level: LevelSide, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockState>> {
        // getStateForPlacement: facing the player. Both sides answer it alike (a state change, no world read).
        let (LevelSide::Server(level) | LevelSide::Client(level)) = level;
        let facing = match event.context.source {
            PlacementSource::Player { yaw, .. } => interact::facing_for(yaw),
            // DirectionalPlaceContext.getHorizontalDirection: its direction, north when vertical; then getOpposite.
            PlacementSource::Directional { direction } => interact::facing_for_direction(direction),
            _ => return None,
        };
        level.state_with_property(event.state, "facing", facing).map(Some)
    }

    fn has_analog_output(&self, _level: &dyn LevelHost, event: &mut HasAnalogOutputEvent) {
        event.output = true;
    }

    fn analog_output(&self, _level: &mut dyn LevelRandomHost, event: &mut AnalogOutputEvent) {
        let key = (event.dimension.key().id().to_owned(), [event.pos.x, event.pos.y, event.pos.z]);
        event.output = with_state(|s| s.comparator.get(&key).copied()).unwrap_or(0);
    }

    fn on_block_exploded(&self, host: &mut dyn ServerHost, event: &mut OnBlockExplodedEvent<'_>) {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Some(level) = host.level(host.dimension()) else { return };
        if !level.block(host, pos).is_some_and(|b| b.id(host).is_some_and(is_rainbow)) {
            return;
        }
        // BlockMillionFurnace.onExplosionHit: a Rainbow Coal two blocks up, the furnace gone.
        with_state(|s| s.exploded.insert(pos));
        if let Some(coal) = KEYS.get().and_then(|k| ItemStack::new(k.rainbow_coal, 1)) {
            let _ = level.spawn_item(host, f64::from(pos[0]), f64::from(pos[1]) + 2.0, f64::from(pos[2]), coal, 10);
        }
    }
}

#[mod_api::sunrise_part]
impl BlockEntityPart for Furnace {
    fn block_entity(&self, _level: LevelSide, _event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
        // (The entity's id is its type, `ironfurnaces:furnace`: the tier is the block's.)
        tick_on_fields(&mut event.subject, |host, pos, entity| {
            let Some(block) =
                host.level(host.dimension()).and_then(|l| l.block(host, pos)).and_then(|b| b.id(host).map(str::to_owned))
            else {
                return;
            };
            furnace::tick(host, pos, &block, entity);
            client::comparator(host, pos, &block, entity);
        });
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        // What the client's copy needs: the page and mode its screen lays out, the look.
        let mut tag = Compound::new();
        for key in ["hz_mode", "AugmentGUI", "Jovial"] {
            if let Some(v) = event.entity.nbt.get(&java(key)) {
                tag.insert(java(key), v.clone());
            }
        }
        Some(tag)
    }

    fn menu_quick_move(&self, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        // quickMoveStack from the player's inventory (`quickmove`); from the furnace's slots, the game's routing (into
        // the inventory, the results from its end) is the Java menu's.
        // The resident entity is lent through views: its fields are copied out, moved on, and what changed written back.
        let Some((handle, pos, before)) = event.subject.with_view(|view| (view.handle(), view.position(), fields_of(&view))) else {
            return Flow::Continue;
        };
        let host = event.subject.host();
        let Some(player) = host.players().acting_player().and_then(|p| p.entity().uuid(host)) else { return Flow::Continue };
        let Some(level) = host.level(host.dimension()) else { return Flow::Continue };
        let Some(block) = level.block(host, pos).and_then(|b| b.id(host).map(str::to_owned)) else { return Flow::Continue };
        let mut entity = before.clone();
        let handled = quickmove::run(host, player, level, &block, &mut entity, event.slot, event.stack);
        if entity != before && event.subject.with_view(|mut view| write_fields(&mut view, &before, &entity)) == Some(true) {
            let _ = handle.set_changed(event.subject.host());
        }
        if handled { Flow::Cancel } else { Flow::Continue }
    }

    fn pre_remove_side_effects(&self, _level: &mut dyn LevelRandomHost, event: &mut PreRemoveSideEffectsEvent<'_>) {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let xp = furnace::stored_experience(&event.entity.nbt);
        with_state(|s| {
            s.xp_on_break.insert(pos, xp);
            s.comparator.retain(|(_, p), _| *p != pos);
        });
    }

    /// A lit furnace's flames, smoke and sounds run on the client, for every loaded copy (`client::animate`).
    fn block_entity_client_ticks(&self, _host: &dyn ClientHost, event: &mut BlockEntityClientTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_client_tick(&self, event: &mut BlockEntityClientTickEvent<'_>) {
        let Some(pos) = event.subject.with_view(|view| view.position()) else { return };
        client::animate(event.subject.host(), pos);
    }
}

#[mod_api::sunrise_part]
impl BlockPart for Heater {
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        interact::heater_use(host, event.click)
    }
}

#[mod_api::sunrise_part]
impl BlockEntityPart for Heater {
    fn block_entity(&self, _level: LevelSide, _event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
        tick_on_fields(&mut event.subject, |_, pos, entity| furnace::heater_tick(pos, entity));
    }

    fn pre_remove_side_effects(&self, _level: &mut dyn LevelRandomHost, event: &mut PreRemoveSideEffectsEvent<'_>) {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let energy = get_i(&event.entity.nbt, "energy", 0);
        with_state(|s| s.heater_on_break.insert(pos, energy));
    }
}

#[mod_api::sunrise_part]
impl ItemPart for Tool {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        // Server-side as before: a claimed use swings the hand and neither the item's `useOn` nor its `use` runs.
        let HostSide::Server(host) = host else { return None };
        matches!(interact::item_use_on(host, event), Flow::Cancel).then_some(InteractionResult::SuccessServer)
    }
}

#[mod_api::sunrise_part]
impl ItemPart for HeaterItem {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(_) = host else { return None };
        // The energy it holds goes to the transmitter it places (`entity_place`).
        let energy = tooltip::heater_energy(event.stack).unwrap_or(0);
        let player = event.player;
        with_state(|s| s.placing_heater.insert(player, energy));
        None
    }

    fn item_bar(&self, _host: &dyn ClientHost, event: &mut ItemBarEvent<'_>) {
        // BlockItemHeater: isBarVisible with energy, 13 pixels for a full store, getBarColor -8387072.
        if let Some(energy) = tooltip::heater_energy(event.stack) {
            *event.bar = Some(((13.0 * (f64::from(energy) / 1_000_000.0)) as i32, (-8_387_072i32) as u32));
        }
    }
}

#[mod_api::sunrise_part]
impl MenuPart for Menus {
    /// Every id of a block's menu reaches `menu_button`, which checks its own ids, as every `clickMenuButton` reached
    /// the Java menu. Main drops a button id its menu's owner doesn't accept.
    fn menu_button_valid(&self, _host: &dyn Host, event: &mut MenuButtonValidEvent<'_>) -> Flow {
        if event.block.is_some() { Flow::Cancel } else { Flow::Continue }
    }

    fn menu_button(&self, event: &mut MenuButtonEvent<'_>) {
        let Some(pos) = event.context.pos else { return };
        let Some((handle, before)) = event.subject.with_view(|view| (view.handle(), fields_of(&view))) else { return };
        let mut entity = before.clone();
        screen::menu_button(event.subject.host(), event.context.player, pos, event.context.id, &mut entity.nbt);
        if entity != before && event.subject.with_view(|mut view| write_fields(&mut view, &before, &entity)) == Some(true) {
            let _ = handle.set_changed(event.subject.host());
        }
    }

    /// SlotIronFurnace.onTake: whoever takes a result gets the furnace's stored experience, as orbs where they stand, and
    /// its recipes (unlockRecipes).
    fn menu_slot_take(&self, event: &mut MenuSlotTakeEvent<'_>) {
        if event.menu != "ironfurnaces:furnace" || !(event.slot == furnace::OUTPUT || (13..=18).contains(&event.slot)) {
            return;
        }
        let Some((handle, before)) = event.subject.with_view(|view| (view.handle(), fields_of(&view))) else { return };
        let mut entity = before.clone();
        let (ids, amount) = furnace::take_experience(&mut entity.nbt);
        if entity != before && event.subject.with_view(|mut view| write_fields(&mut view, &before, &entity)) == Some(true) {
            let _ = handle.set_changed(event.subject.host());
        }
        let host = event.subject.host();
        let Some(player) = host.player(event.player) else { return };
        furnace::award(host, player, &ids, amount);
    }

    fn menu_slot_rules(&self, _host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        let Some(keys) = KEYS.get() else { return };
        if event.container != MenuContainer::Main {
            return;
        }
        let item = event.stack.item();
        if event.menu == "ironfurnaces:heater" {
            // SlotHeater: only a receiver.
            event.may_place = item == keys.item_heater;
            return;
        }
        if event.menu != "ironfurnaces:furnace" {
            return;
        }
        let augment = keys.augments.iter().find(|(k, _)| *k == item).map(|(_, slot)| *slot);
        // The red augment in the menu (the furnace's recipe type, the generator's fuel), as the Java slots read the
        // block entity's slot 3.
        let red = event.context.slot(MenuContainer::Main, furnace::AUGMENT_RED).map(|s| s.item());
        let red = recipes::red_augment(red, keys.augments[0].0, keys.augments[1].0);
        // What an item is, worked out when the data packs loaded; until then nothing is held back.
        let sets = recipes::sets();
        let is = |set: fn(&recipes::ItemSets) -> &std::collections::HashSet<ItemKey>| sets.as_ref().is_none_or(|s| set(s).contains(&item));
        match event.slot {
            // SlotIronFurnaceInput and SlotIronFurnaceInputFactory: hasRecipe, of the furnace's own type.
            furnace::INPUT | 7..=12 => {
                event.may_place = sets.as_ref().is_none_or(|s| s.cookable[red.kind().index()].contains(&item));
            }
            // SlotIronFurnaceFuel: a fuel (a receiver too), or a bucket, one at a time.
            furnace::FUEL => {
                if item == keys.bucket {
                    event.max_stack = 1;
                } else {
                    event.may_place = item == keys.item_heater || is(|s| &s.fuel);
                }
            }
            // SlotIronFurnace and SlotIronFurnaceOutputFactory: results only come out.
            furnace::OUTPUT | 13..=18 => event.may_place = false,
            // The augment slots: their colour's augment, one.
            furnace::AUGMENT_RED..=furnace::AUGMENT_BLUE => {
                event.may_place = augment == Some(event.slot);
                event.max_stack = 1;
            }
            // SlotIronFurnaceInputGenerator: by the red augment, a generator_blasting recipe's item, a food, or a
            // fuel that is no receiver.
            furnace::GENERATOR_FUEL => {
                event.may_place = match red {
                    recipes::Red::Blasting => is(|s| &s.generator),
                    recipes::Red::Smoking => is(|s| &s.food),
                    recipes::Red::None => item != keys.item_heater && is(|s| &s.fuel),
                };
            }
            _ => {}
        }
    }
}
