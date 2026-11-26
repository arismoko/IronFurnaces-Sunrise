# Iron Furnaces for Sunrise

This is Iron Furnaces 4.4.2 (Qelifern, XenoMustache, Astryxion) as a native Sunrise mod. It uses Sunrise's typed mod API
only: hooks and Parts, with no `[native.*]` rows. The Rust code is a rewrite, checked against the decompiled 4.4.2 jar.
The textures, models, recipes, loot tables and language files are the original mod's.

The build is pinned to Sunrise Dev 2026-11-25, commit `6a02076f89` (`sunrise-mod.toml`). The optional dependencies are:

- JEI (`^0.5`): the generator categories.
- AE2 (`^15.4.13`): `ae2::api::register_machine`, for its crafting ETA.
- Machine Work (`^0.1`): the shared `machine_work::MACHINE_WORK` capability.
- GuideME (`^0.1`): the `/ironfurnaces` menu and the pages in `assets/ironfurnaces/ae2guide`. GuideME isn't published
  for Sunrise yet, so these do nothing for now.

Each one turns on the Cargo feature named after it when it is installed.

## Layout

- `sunrise-mod.toml`, `Cargo.toml`, `src/`: the mod.
  - `lib.rs`: hooks, Parts (the furnaces, the transmitter, the upgrade, linker and transmitter items, the menus) and
    shared state.
  - `furnace.rs`: BlockIronFurnaceTileBase. Covers the furnace, factory and generator ticks, augments, auto input and
    output, redstone, experience, the Rainbow Furnace's generation and the transmitter's binding.
  - `caps.rs`: the furnaces' `ITEMS` and `ENERGY` capabilities (side settings, slot validity, energy by mode).
  - `interact.rs`: right clicks, augments, the looks, the Settings Linker, upgrades and placement facing.
  - `screen.rs`: the furnace, factory, generator and augment pages, the config tab and the transmitter's screen.
  - `quickmove.rs`: shift-clicks (quickMoveStack).
  - `recipes.rs`: cooking recipes from the world, the generator recipes and the slot-rule item sets.
  - `owners.rs`: PlayerFurnacesList, as a player attachment.
  - `tooltip.rs`: the items' tooltips.
  - `client.rs`: animateTick (flames, smoke, sounds) and the comparator.
  - `config.rs`: `config/ironfurnaces.toml`, with the Java mod's keys.
  - `jei.rs`: the JEI plugin.
  - `work.rs`: the furnaces' `machine_work::MACHINE_WORK` answer. AE2's crafting ETA reads it first.
  - `ae2_eta.rs`: the same times through `ae2::api::register_machine`, for an AE2 without the library.
  - `commands_menu.rs`: `/ironfurnaces`.
- `assets/` and `data/`: the jar's files as they are, plus Sunrise's declarations under `data/ironfurnaces/sunrise/`.
  `data/c/` also holds 16 convention tags that the original took from NeoForge and Sunrise doesn't seed on its own (see below).

## Build

`cargo sunrise-mod check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` run against the pinned SDK in
`.sunrise/sdk`, which `cargo sunrise-mod` fetches. Releases go out with `cargo sunrise-mod release`.

## On the Sunrise APIs

- **Energy** is Sunrise's `ENERGY` capability. The furnaces answer it themselves, before the game's own answer, so the
  storage behaves as the Java tick sets it:
  - a factory takes energy and gives none;
  - a generator gives energy and takes none;
  - a furnace does neither.

  Generators push through their neighbours' `ENERGY` handlers in transactions. The transmitter is a `ModEnergyBuffer`.
  A furnace takes 2000 from its bound transmitter by writing the transmitter's saved `energy`, as the Java mod's
  `removeEnergy` does.
- **Items** are Sunrise's `ITEMS` capability, with WorldlyContainer rules: which slots each side's setting reaches, what
  fits there and what may come out. Auto input and output move through the neighbours' `ITEMS` handlers in
  transactions.
- **Menus** are content menu types with a `MenuPart`:
  - `menu_slot_rules` answers the slot rules from item sets worked out when the data packs load.
  - `menu_quick_move` is quickMoveStack.
  - `menu_slot_take` is SlotIronFurnace.onTake: taking a result gives its experience as orbs and unlocks its recipes.
  - The screen moves the slots with `set_screen_slot` when the page or the mode changes, as the Java slots'
    `isActive` does.
- **The player's furnace list** is an attachment saved with the player.
- **Light:** the blocks declare `dynamic_light`. `light_emission` gives 0 while `misc.lightupdates` is on, as in the
  Java mod; otherwise a lit furnace gives 14.
- **Flames, smoke and their sounds** come from the client block entity tick, at vanilla's random display tick rate.
- **Experience from a broken furnace** is taken in `pre_remove_side_effects` and added in `block_drops`, as orbs at the
  block (grantStoredRecipeExperience).
- **Writes to the held stack** (a Settings Linker copy, or an augment or upgrade used) go through the hand's inventory
  slot, as the Java code changes the stack in hand.

## Differences from the original

Each comes from a piece the mod API doesn't have yet, filed on Sunrise:

- **Convention tags.** Sunrise seeds the `c:` tags for copper, iron and gold ingots, diamonds, obsidians, colourless
  glass and gold blocks, so the recipes use them from the game (sunrise-org/sunrise#1244, now merged). `data/c/` keeps
  only the tags the original declared beyond those (the metals' and gems' other entries, the furnace tags, and the
  empty Silver, Allthemodium, Vibranium and Unobtainium ones). The recipes of those absent metals carry the original's
  `neoforge:tag_empty` conditions, which Sunrise answers at startup (sunrise-org/sunrise#5491, now merged).
- **Item data.** The Settings Linker's settings and the transmitter item's energy live in `minecraft:custom_data`. The
  Java mod registers its own component types (sunrise-org/sunrise#1243).
- **Comparator.** The comparator output is computed in the furnace's tick and kept for `analog_output`, so it trails a
  change of contents by up to a tick. `analog_output` can't read an item's maximum stack size
  (sunrise-org/sunrise#5473).
- **JEI cards.** In JEI, a generator card shows its energy as text under the card, and its flame and energy bar are not
  animated. This is a limit of the JEI port's recipe API, not of Sunrise's.

These match the Java mod on purpose:

- Upgrading an Iron Furnace doesn't carry over its augment record, so a furnace with a blue augment drops its contents
  on the first tick after the upgrade.
- A blast leaves a Rainbow Furnace's Rainbow Coal, not its core.
