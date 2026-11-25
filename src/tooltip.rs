//! The items' appendHoverText: furnaces' cook time (and the Rainbow Furnace's generation), augments, upgrades, the
//! receiver's binding, the transmitter's energy, the looks and the Settings Linker. Client only.

use crate::furnace::heater_position;
use crate::screen::{energy_text, grouped};
use crate::util::*;
use crate::{config, interact};
use mod_api::text::TextStyle;
use mod_api::*;

fn styled(text: String, colour: &str, italic: bool) -> Text {
    let mut style = TextStyle::default();
    style.color = Some(colour.to_owned());
    if italic {
        style.italic = Some(true);
    }
    Text::literal(text).with_style(style)
}

fn shift(host: &dyn ClientHost) -> bool {
    host.is_key_down("key.keyboard.left.shift") || host.is_key_down("key.keyboard.right.shift")
}

/// StringHelper.getShiftInfoText: "Hold [Shift] for Details."
fn shift_info(host: &dyn ClientHost) -> Text {
    styled(
        lang(host, "tooltip.ironfurnaces.hold", "Hold "),
        "gray",
        false,
    )
    .append(styled("[Shift]".to_owned(), "gold", true))
    .append(styled(
        lang(host, "tooltip.ironfurnaces.for_details", " for Details."),
        "gray",
        false,
    ))
}

pub(crate) fn tooltip(host: &dyn ClientHost, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let item = id_of(host, stack);
    let Some(name) = item.strip_prefix("ironfurnaces:") else {
        return;
    };
    let mut add = |text: Text| lines.push((text, u32::MAX));
    let t = |key: &str, english: &str| lang(host, &format!("tooltip.ironfurnaces.{key}"), english);
    if name.ends_with("_furnace") {
        add(styled(
            format!(
                "{} ({})",
                t("cooktime", "Cooktime"),
                config::speed(host, &item)
            ),
            "blue",
            false,
        ));
        if name == "million_furnace" {
            let generation = grouped(config::cfg(host, "rainbow_furnace.rainbow_generation"));
            add(styled(
                format!(
                    "{} {generation} {}",
                    t("rainbow_gen1", "Will generate"),
                    t(
                        "rainbow_gen2",
                        "RF/tick if all other furnaces are generating power"
                    )
                ),
                "gray",
                false,
            ));
            add(styled(
                t("rainbow_blowup", "Something is telling me to blow it up..."),
                "gold",
                true,
            ));
            if shift(host) {
                add(styled(
                    t(
                        "rainbow_gen3",
                        "For power generation, one of each tier of furnace must be an active generator and not have a full buffer",
                    ),
                    "gray",
                    false,
                ));
                add(styled(
                    t(
                        "rainbow_gen4",
                        "Outputs energy to any side that is configured as output, it wont add energy to the internal buffer",
                    ),
                    "gray",
                    false,
                ));
                add(styled(
                    t("rainbow_gen5", "You can only have one Rainbow Generator"),
                    "gray",
                    false,
                ));
            } else {
                add(shift_info(host));
            }
        }
        return;
    }
    let augment = |key: &str, english: &str| (key.to_owned(), english.to_owned());
    let (lines_for, colours): (Vec<(String, String)>, Vec<&str>) = match name {
        "augment_blasting" => (
            vec![augment(
                "augment_blasting",
                "Only allows for blasting recipes.",
            )],
            vec!["gold"],
        ),
        "augment_smoking" => (
            vec![augment(
                "augment_smoking",
                "Only allows for smoking recipes.",
            )],
            vec!["gold"],
        ),
        "augment_speed" => (
            vec![
                augment("augment_speed_pro", "+Halves the cooktime for all recipes."),
                augment("augment_speed_con", "-Uses up twice the amount of fuel."),
            ],
            vec!["green", "dark_red"],
        ),
        "augment_fuel" => (
            vec![
                augment(
                    "augment_fuel_pro",
                    "+Fuel heats up the furnace twice as much.",
                ),
                augment(
                    "augment_fuel_con",
                    "-Slows down the cooktime by 25% for all recipes.",
                ),
            ],
            vec!["green", "dark_red"],
        ),
        "augment_factory" => (
            vec![
                augment(
                    "augment_factory_pro",
                    "+Converts the furnace into a factory.",
                ),
                augment(
                    "augment_factory_con",
                    "-Uses power for heat instead of fuel.",
                ),
            ],
            vec!["green", "dark_red"],
        ),
        "augment_generator" => (
            vec![
                augment(
                    "augment_generator_pro",
                    "+Heat from the furnace will generate power.",
                ),
                augment(
                    "augment_generator_con",
                    "-The furnace can no longer smelt items.",
                ),
            ],
            vec!["green", "dark_red"],
        ),
        _ => (Vec::new(), Vec::new()),
    };
    if name.starts_with("augment_") {
        add(styled(
            t("augment_right_click", "Right-click to insert augment"),
            "gray",
            false,
        ));
        for ((key, english), colour) in lines_for.iter().zip(colours) {
            add(styled(t(key, english), colour, false));
        }
        return;
    }
    match name {
        "item_heater" => {
            if !shift(host) {
                add(shift_info(host));
            } else if let Some(p) = heater_position(stack) {
                add(styled(t("heater", "Bound to: "), "gray", false));
                add(styled(
                    format!("{}{}", t("heaterX", "X: "), p[0]),
                    "gray",
                    false,
                ));
                add(styled(
                    format!("{}{}", t("heaterY", "Y: "), p[1]),
                    "gray",
                    false,
                ));
                add(styled(
                    format!("{}{}", t("heaterZ", "Z: "), p[2]),
                    "gray",
                    false,
                ));
            } else {
                add(styled(
                    t(
                        "heater_not_bound",
                        "Has yet to be bound to an energy source!",
                    ),
                    "gray",
                    false,
                ));
                add(styled(
                    t("heater_tip", "Place in the fuel slot of the furnace"),
                    "gray",
                    false,
                ));
                add(styled(
                    t("heater_tip1", "Only works from Iron tier and above"),
                    "gray",
                    false,
                ));
            }
        }
        "heater" => {
            if let Some(energy) = heater_energy(stack) {
                add(styled(energy_text(energy, 1_000_000), "gold", false));
            }
            if shift(host) {
                add(styled(
                    t("heater_block", "Link with a Wireless Heat Receiver"),
                    "gray",
                    false,
                ));
                add(styled(
                    t("heater_block1", "Provide this block with Forge Energy"),
                    "gray",
                    false,
                ));
            } else {
                add(shift_info(host));
            }
        }
        "item_xmas" => {
            add(styled(
                t(
                    "xmas_right_click",
                    "Right-click on furnace to wrap it as a gift!",
                ),
                "gray",
                false,
            ));
            add(styled(
                t("xmas1", "But won't the wrappings catch fire?"),
                "gold",
                true,
            ));
            add(styled(
                t(
                    "xmas2",
                    "Sneaking and right-clicking on a furnace opens the gift",
                ),
                "gray",
                false,
            ));
        }
        "item_spooky" => {
            add(styled(
                t("spooky_right_click", "Right-click on furnace to spookalize"),
                "gray",
                false,
            ));
            add(styled(t("spooky1", "3spooky5me"), "gold", true));
            add(styled(
                t(
                    "spooky2",
                    "Sneaking and right-clicking on a furnace will cleanse any spookalizations",
                ),
                "gray",
                false,
            ));
        }
        "item_copy" => {
            // (The Java tooltip reads a "settings" array of ten that the linker never writes: only these show.)
            let _ = interact::linker_settings(stack);
            add(styled(
                "Right-click to copy settings".to_owned(),
                "gray",
                false,
            ));
            add(styled(
                "Sneak & right-click to apply settings".to_owned(),
                "gray",
                false,
            ));
        }
        _ if interact::upgrade_of(&item).is_some() => {
            add(styled(
                t("upgrade_right_click", "Sneak & right-click to upgrade"),
                "gray",
                false,
            ));
        }
        _ => {}
    }
}

/// The transmitter item's stored energy (Registration.ENERGY, kept in its custom data here).
pub(crate) fn heater_energy(stack: &ItemStack) -> Option<i32> {
    let c = custom_data(stack)?;
    c.get(&java("Energy"))?;
    Some(get_i(&c, "Energy", 0))
}
