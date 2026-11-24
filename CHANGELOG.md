# Changelog

## 4.4.13 (2026-11-24)

- Shift-clicking a whole stack empties its slot; the stand-in item that used to fill it is gone.
- Taking a result gives its experience and recipes at once.
- The augment page and the factory and generator modes move the slots in place instead of reopening the screen.
- The recipes now equal the original's; only their load conditions wait for a Sunrise fix (sunrise-org/sunrise#5491).

## 4.4.9 (2026-11-21)

- A newly placed furnace's first screen now shows its slots: items put in or taken out showed only after reopening it.

## 4.4.8 (2026-11-20)

- Updated for Sunrise Nightly 2026-11-20 (`9425651`): furnace and heater screens now attach to their menu types.
- The SDK-built package includes the Apache-2.0 `LICENSE` and `NOTICE`.

## 4.4.7 (2026-11-19)

- Pinned to the Sunrise Nightly of 2026-11-19 (`ed3ac19`): screen tooltips use the new rich-text lines, placement and
  comparator output the new hook signatures.
- The furnaces' loot tables are in 26.3's form, so their drop conditions and functions apply (the Nightly reads only
  that form).

## 4.4.6 (Sunrise)

- The W guide rewritten: 41 short pages covering every furnace tier, upgrades, augments, factories, generators, wireless heat, automation, the config file, and the other mods (AE2, its crafting ETA, Machine Work, JEI). Every item opens its own page with W.
- In English, Spanish and German, with 24 3D scenes and 58 recipe boxes.

## 4.4.5 (Sunrise)

First release for Sunrise (Nightly 2026-11-17, `2d81263`, Minecraft 26.3), moved from the Horizon port.

- Every furnace tier, the Rainbow Furnace, the Heater, augments (speed, fuel, factory, generator, blasting, smoking), factories, generators, the Furnace Transmitter, the Settings Linker and furnace upgrades.
- Auto input and output, side settings, redstone modes and experience, through Sunrise's `ITEMS` and `ENERGY` capabilities, so pipes and other mods' blocks reach the furnaces.
- Flames, smoke and their sounds, light, and the comparator.
- With JEI: the generator categories and catalysts. With GuideME: the W guide.
- With AE2: the crafting ETA reads each furnace's real time per craft (tier, augments, factory rows). With Machine Work: the furnaces answer the shared `machine_work::MACHINE_WORK` capability.
