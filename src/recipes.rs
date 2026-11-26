//! The recipes a furnace reads: the world's smelting, blasting and smoking (`LevelRef::recipe_for`, data packs
//! included, as a vanilla furnace finds them), and Iron Furnaces' own `ironfurnaces:generator_blasting`
//! (`recipes_of_type`, matched here). Both are kept until the data packs change (`tags_updated`, after a world loads and
//! after `/reload`), so a ticking furnace doesn't ask the game for the same recipe every tick.

use crate::util::*;
use mod_api::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Smelting,
    Blasting,
    Smoking,
}

impl Kind {
    /// Its place in `ItemSets::cookable`.
    pub(crate) fn index(self) -> usize {
        match self {
            Kind::Smelting => 0,
            Kind::Blasting => 1,
            Kind::Smoking => 2,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Kind::Smelting => "minecraft:smelting",
            Kind::Blasting => "minecraft:blasting",
            Kind::Smoking => "minecraft:smoking",
        }
    }
}

/// What a cooking recipe makes of one input.
#[derive(Clone, Debug)]
pub(crate) struct Cooked {
    pub id: String,
    pub result: ItemStack,
    pub experience: f32,
    pub time: u32,
}

/// Each furnace kind's smelting result for an input item, or `None` when it has none.
type CookedCache = HashMap<(Kind, ItemKey), Option<Cooked>>;

static COOKED: Mutex<Option<CookedCache>> = Mutex::new(None);
/// The generator recipes: what each takes (item ids) and the energy it gives.
static GENERATOR: Mutex<Option<Vec<(Ingredient, i32)>>> = Mutex::new(None);

/// Forget what was looked up: the data packs changed.
pub(crate) fn forget() {
    *COOKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *GENERATOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *SETS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// What each kind of item is, for the menu's slot rules (asked on both sides with the read host and the menu's
/// slots, so the recipe reads are worked out beforehand, each time the data packs load): what each cooking type's
/// recipes take (SlotIronFurnaceInput.mayPlace: hasRecipe of the furnace's type), the fuels (isItemFuel), the foods
/// (getSmokingBurn) and what the generator recipes take.
#[derive(Default)]
pub(crate) struct ItemSets {
    /// By `Kind`: smelting, blasting, smoking.
    pub cookable: [HashSet<ItemKey>; 3],
    pub fuel: HashSet<ItemKey>,
    pub food: HashSet<ItemKey>,
    pub generator: HashSet<ItemKey>,
}

/// The furnace's red augment, as its slot rules read it from the menu's slot 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Red {
    None,
    Blasting,
    Smoking,
}

impl Red {
    /// The recipe type the furnace cooks with it (BlockIronFurnaceTileBase.recipeType).
    pub(crate) fn kind(self) -> Kind {
        match self {
            Red::None => Kind::Smelting,
            Red::Blasting => Kind::Blasting,
            Red::Smoking => Kind::Smoking,
        }
    }
}

/// Which red augment `red` (the item in slot 3, if any) is, given the blasting and smoking augments' items.
pub(crate) fn red_augment<K: PartialEq>(red: Option<K>, blasting: K, smoking: K) -> Red {
    match red {
        Some(k) if k == blasting => Red::Blasting,
        Some(k) if k == smoking => Red::Smoking,
        _ => Red::None,
    }
}

static SETS: Mutex<Option<Arc<ItemSets>>> = Mutex::new(None);

pub(crate) fn sets() -> Option<Arc<ItemSets>> {
    SETS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Works the sets out if they aren't yet (after a world loads, after `/reload`).
pub(crate) fn ensure_sets(host: &dyn Host) {
    if sets().is_some() {
        return;
    }
    let mut out = ItemSets::default();
    let key = |id: &str| item_key(host, id);
    let mut any = false;
    for kind in [Kind::Smelting, Kind::Blasting, Kind::Smoking] {
        for r in host.recipes_of_type(kind.id()).iter() {
            any = true;
            for ingredient in &r.ingredients {
                out.cookable[kind.index()].extend(ingredient.items.iter().filter_map(|i| key(i)));
            }
        }
    }
    if !any {
        // Outside a world: nothing to go on yet.
        return;
    }
    for item in host.item_keys() {
        let id = item.id(host);
        if id == Some("ironfurnaces:rainbow_coal") || host.item_burn_time(item).unwrap_or(0) > 0 {
            out.fuel.insert(item);
        }
        if let Some(Tag::Compound(food)) = host.item_default_component(item, "minecraft:food")
            && get_i(&food, "nutrition", 0) > 0
        {
            out.food.insert(item);
        }
    }
    for (ids, _) in generator_all(host) {
        out.generator.extend(ids.iter().filter_map(|i| key(i)));
    }
    *SETS.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(out));
}

fn lookup(host: &dyn Host, level: LevelRef, kind: Kind, stack: &ItemStack) -> Option<Cooked> {
    let id = Identifier::parse(kind.id()).ok()?;
    let one = with_count(stack, 1)?;
    let found = level
        .recipe_for(host, &id, &RecipeInput::Single(one))
        .ok()
        .flatten()?;
    Some(Cooked {
        id: found.recipe.to_string(),
        result: found.output,
        experience: found.experience,
        // A recipe file without `cookingtime` has its type's default (AbstractCookingRecipe's 200, or 100 for
        // blasting and smoking), which the match leaves at 0.
        time: if found.processing_time > 0 {
            found.processing_time
        } else if kind == Kind::Smelting {
            200
        } else {
            100
        },
    })
}

/// The `kind` recipe for `stack`, if any.
pub(crate) fn cook(
    host: &dyn Host,
    level: LevelRef,
    kind: Kind,
    stack: &ItemStack,
) -> Option<Cooked> {
    if stack.components().is_some() {
        // A stack with its own components may match differently: asked every time.
        return lookup(host, level, kind, stack);
    }
    let key = (kind, stack.item());
    if let Some(found) = COOKED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get(&key)
    {
        return found.clone();
    }
    let found = lookup(host, level, kind, stack);
    COOKED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key, found.clone());
    found
}

fn generator_recipes(host: &dyn Host) -> Vec<(Ingredient, i32)> {
    let mut guard = GENERATOR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(list) = guard.as_ref() {
        return list.clone();
    }
    let mut list = Vec::new();
    for r in host
        .recipes_of_type("ironfurnaces:generator_blasting")
        .iter()
    {
        let Some(ingredient) = r.json.get("ingredient") else {
            continue;
        };
        let Ok(Some(ingredient)) = host.recipe_ingredient(&ingredient.to_string()) else {
            continue;
        };
        let energy = r.json.get("energy").and_then(|e| e.as_i64()).unwrap_or(0) as i32;
        list.push((ingredient, energy));
    }
    // Outside a world there are none yet: not kept, so the first look in one reads them.
    if !list.is_empty() {
        *guard = Some(list.clone());
    }
    list
}

/// `ironfurnaces:generator_blasting`'s energy for a stack (GeneratorRecipe.getEnergy).
pub(crate) fn generator_energy(host: &dyn Host, stack: &ItemStack) -> Option<i32> {
    let id = id_of(host, stack);
    generator_recipes(host)
        .into_iter()
        .find_map(|(ingredient, energy)| ingredient.items.contains(&id).then_some(energy))
}

/// Every generator recipe: the items it takes, and its energy (the JEI-style tab and the guide).
pub(crate) fn generator_all(host: &dyn Host) -> Vec<(Vec<String>, i32)> {
    generator_recipes(host)
        .into_iter()
        .map(|(i, e)| (i.order.clone(), e))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_augment_picks_the_cooking_type() {
        assert_eq!(red_augment(None, 1, 2).kind(), Kind::Smelting);
        assert_eq!(red_augment(Some(1), 1, 2).kind(), Kind::Blasting);
        assert_eq!(red_augment(Some(2), 1, 2).kind(), Kind::Smoking);
        // Another item in the slot (none can be placed there, but a saved one could be) cooks as a plain furnace.
        assert_eq!(red_augment(Some(3), 1, 2), Red::None);
        assert_eq!(Kind::Blasting.index(), 1);
    }

    #[test]
    fn absent_metal_recipes_keep_their_tag_empty_conditions() {
        let recipes = [
            include_str!("../data/ironfurnaces/recipe/furnaces/silver_furnace.json"),
            include_str!("../data/ironfurnaces/recipe/furnaces/copper_furnace.json"),
            include_str!("../data/ironfurnaces/recipe/rainbow_plating.json"),
            include_str!("../data/ironfurnaces/recipe/upgrades/upgrade_silver.json"),
        ];
        for json in recipes {
            assert!(json.contains("\"neoforge:conditions\""));
            assert!(json.contains("\"neoforge:tag_empty\""));
        }
    }
}
