//! IronFurnacesJEIPlugin through the JEI port's API (`jei::api`): the three generator categories
//! (RecipeCategoryGeneratorRegular, Blasting and Smoking: an item, and the energy it gives) and the catalysts (the
//! furnaces and the factory augment for smelting, the blasting and smoking augments for theirs, the generator augment
//! for the generators'). Registered on the client, only where JEI is installed.

#[cfg(feature = "jei")]
use crate::screen::grouped;
#[cfg(feature = "jei")]
use crate::util::*;
#[cfg(feature = "jei")]
use mod_api::*;

#[cfg(feature = "jei")]
pub(crate) fn register(host: &dyn ClientHost) {
    use jei::api::*;
    const GENERATOR: &str = "ironfurnaces:augment_generator";
    register_category(
        CategoryDef::new("ironfurnaces:generator_blasting", "ironfurnaces.jei_category_blasting", GENERATOR)
            .catalyst("ironfurnaces:augment_blasting"),
    );
    register_category(
        CategoryDef::new("ironfurnaces:generator_smoking", "ironfurnaces.jei_category_smoking", GENERATOR)
            .catalyst("ironfurnaces:augment_smoking"),
    );
    register_category(CategoryDef::new("ironfurnaces:generator_regular", "ironfurnaces.jei_category_regular", GENERATOR));
    add_catalyst("minecraft:blasting", "ironfurnaces:augment_blasting");
    add_catalyst("minecraft:smoking", "ironfurnaces:augment_smoking");
    add_catalyst("minecraft:smelting", "ironfurnaces:augment_factory");
    let atm = item_key(host, "allthemodium:allthemodium_ingot").is_some();
    for furnace in crate::furnace::FURNACES {
        // The Allthemodium tiers only with Allthemodium's content (Registration.isAtmContentAvailable).
        let other = ["allthemodium", "vibranium", "unobtainium"].iter().any(|m| furnace.contains(m));
        if !other || atm {
            add_catalyst("minecraft:smelting", furnace);
        }
    }
    register_recipes("ironfurnaces", recipes);
}

/// A card as the Java categories draw it: jei.png's 68 by 42, the item at 1,18, and the energy (the Java card shows
/// it as the energy bar's tooltip; the API has text on the card, so it is written under the picture, 10 more pixels).
#[cfg(feature = "jei")]
fn card(category: &str, key: &str, items: &[(&str, u32)], energy: i64) -> (String, jei::api::RecipeDef) {
    let id = format!("{category}/{}", key.replace(':', "/"));
    (
        category.to_owned(),
        jei::api::RecipeDef::new(&id, [68.0, 52.0])
            .background("ironfurnaces:gui/jei", [0.0, 0.0, 68.0, 42.0])
            .input(1.0, 18.0, items)
            .text(1.0, 44.0, &format!("{} RF", grouped(energy.clamp(0, i64::from(i32::MAX)) as i32))),
    )
}

/// registerRecipes: every fuel (burn time × 20), every generator_blasting recipe, every food (smoking burn × 40).
#[cfg(feature = "jei")]
fn recipes(host: &dyn Host) -> Vec<(String, jei::api::RecipeDef)> {
    let mut out = Vec::new();
    let mut keys = host.item_keys();
    keys.sort_by(|a, b| a.id(host).cmp(&b.id(host)));
    for item in &keys {
        let id = item.id(host).unwrap_or_default();
        let burn = if id == "ironfurnaces:rainbow_coal" { 200 } else { host.item_burn_time(*item).unwrap_or(0) };
        if burn > 0 {
            out.push(card("ironfurnaces:generator_regular", &id, &[(&id, 1)], i64::from(burn) * 20));
        }
    }
    for (i, (items, energy)) in crate::recipes::generator_all(host).into_iter().enumerate() {
        let items: Vec<(&str, u32)> = items.iter().map(|i| (i.as_str(), 1)).collect();
        out.push(card("ironfurnaces:generator_blasting", &i.to_string(), &items, i64::from(energy)));
    }
    for item in &keys {
        if let Some(Tag::Compound(food)) = host.item_default_component(*item, "minecraft:food") {
            let nutrition = get_i(&food, "nutrition", 0);
            if nutrition > 0 {
                let id = item.id(host).unwrap_or_default();
                out.push(card("ironfurnaces:generator_smoking", &id, &[(&id, 1)], i64::from(nutrition) * 800 * 40));
            }
        }
    }
    out
}

#[cfg(not(feature = "jei"))]
#[allow(dead_code)]
pub(crate) fn register(_host: &dyn mod_api::ClientHost) {}
