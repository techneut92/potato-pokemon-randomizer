//! The manual-edit rules of each handler on real ROMs from `PPR_TEST_ROMS`: the game's own data fits its rules, a few
//! good edits pass validation and are written, and edits that break a rule are reported. Ignored by default:
//! `PPR_TEST_ROMS=… cargo test -p ppr-romio --test edit_rules_real_roms -- --ignored`.

use std::path::PathBuf;

use ppr_model::{
    Edit, EditIssue, EditList, EditRules, EditSeverity, FieldItemEdit, GameData, ObjectRef, ShopEdit, ShopSize,
    TradeEdit, TrainerEdit, TrainerPokemonEdit, WildLevels, WildSetEdit, WildSlotEdit,
};
use ppr_romio::GameHandler;

/// The supported ROMs in `PPR_TEST_ROMS`, opened.
fn roms() -> Vec<(PathBuf, Box<dyn GameHandler>)> {
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("PPR_TEST_ROMS is readable")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.is_file() && !p.to_string_lossy().contains("randomized"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| ppr_romio::open(&p).ok().map(|h| (p, h)))
        .collect()
}

fn trainer_edit(data: &GameData, index: u16) -> Option<TrainerEdit> {
    let t = data.trainers.iter().find(|t| t.index == index)?;
    Some(TrainerEdit {
        index,
        pokemon: t
            .pokemon
            .iter()
            .map(|p| TrainerPokemonEdit {
                species: p.species,
                level: p.level,
                held_item: p.held_item,
                moves: (!p.reset_moves && p.moves.iter().any(|&m| m != 0)).then_some(p.moves),
            })
            .collect(),
    })
}

/// An edit of every trainer, wild set, trade and shop that sets what the game has.
fn unchanged(data: &GameData) -> Vec<Edit> {
    let trainers = data
        .trainers
        .iter()
        .filter(|t| !t.pokemon.is_empty())
        .filter_map(|t| trainer_edit(data, t.index).map(Edit::Trainer));
    let wild = data.encounters.iter().enumerate().map(|(index, s)| {
        Edit::WildSet(WildSetEdit {
            index,
            slots: s
                .encounters
                .iter()
                .map(|e| WildSlotEdit {
                    species: e.species,
                    min_level: e.min_level,
                    max_level: e.max_level,
                })
                .collect(),
        })
    });
    let trades = data.trades.iter().enumerate().map(|(index, t)| {
        Edit::Trade(TradeEdit {
            index,
            requested: t.requested,
            given: t.given,
            nickname: t.nickname.clone(),
            ot_name: t.ot_name.clone(),
            ot_id: t.ot_id,
            ivs: t.ivs.clone(),
            held_item: t.held_item,
        })
    });
    let shops = data.shops.iter().enumerate().map(|(index, s)| {
        Edit::Shop(ShopEdit {
            index,
            items: s.items.clone(),
        })
    });
    trainers.chain(wild).chain(trades).chain(shops).collect()
}

fn validate(data: &GameData, rules: &EditRules, edits: Vec<Edit>) -> Vec<EditIssue> {
    let list = EditList {
        edits,
        ..EditList::new(&data.identity)
    };
    list.validate(data, rules).expect("the list is for this game")
}

/// The rules each generation is known to have.
fn check_generation_rules(data: &GameData, rules: &EditRules) {
    let name = &data.identity.name;
    match data.identity.generation.number() {
        1 | 2 => {
            assert!(matches!(
                rules.shops.size,
                ShopSize::Grow { max: Some(255) } | ShopSize::Shrink
            ));
            assert!(
                rules.wild.levels.iter().take(5).all(|&l| l == WildLevels::Single),
                "{name}"
            );
            if data.is_yellow() {
                let link = rules
                    .starter_links
                    .first()
                    .expect("Yellow's rival has the second starter");
                let rival = data.trainers.iter().find(|t| t.index == link.trainer).expect("rival");
                assert_eq!(rival.pokemon[link.slot].species, data.starters[1], "{name}");
            }
        }
        3 => {
            assert_eq!(rules.shops.size, ShopSize::Grow { max: None });
            if name.starts_with("Emerald") {
                let steven = data.trainers.last().expect("Mossdeep Steven");
                let limits = rules.trainer(steven.index).expect("listed");
                assert_eq!(
                    (limits.min_pokemon, limits.max_pokemon, limits.held_items),
                    (3, 3, false)
                );
            }
        }
        4 => {
            assert!(matches!(
                rules.shops.size,
                ShopSize::Shared {
                    only_changed: false,
                    ..
                }
            ));
            assert!(rules.wild.levels.contains(&WildLevels::Single), "{name}: grass");
            assert!(rules.wild.levels.contains(&WildLevels::Range), "{name}: water");
        }
        5 => {
            if name.starts_with("Black 2") || name.starts_with("White 2") {
                assert_eq!(rules.shops.size, ShopSize::Grow { max: Some(255) });
                assert!(
                    rules.trainers.trainers.iter().any(|t| !t.levels),
                    "{name}: the tournament sets its levels"
                );
            } else {
                assert!(matches!(rules.shops.size, ShopSize::Shared { only_changed: true, .. }));
            }
        }
        6 => assert_eq!(rules.shops.size, ShopSize::Fixed),
        _ => {
            assert_eq!(rules.shops.size, ShopSize::Fixed);
            assert!(rules.wild.levels.iter().all(|&l| l == WildLevels::Shared), "{name}");
        }
    }
}

/// Edits that break a rule of this game, each with the object its error must point at.
fn bad_edits(data: &GameData, rules: &EditRules) -> Vec<(Edit, ObjectRef)> {
    let mut bad = Vec::new();
    let shop_items = |n: usize| -> Vec<u16> { data.shops[0].items.iter().copied().cycle().take(n).collect() };
    let too_many = match &rules.shops.size {
        ShopSize::Fixed | ShopSize::Shrink => data.shops.first().map(|s| s.items.len() + 1),
        ShopSize::Grow { max } => max.map(|m| m + 1),
        ShopSize::Shared { max, .. } => Some(max + 1),
    };
    if let Some(n) = too_many.filter(|_| !data.shops.is_empty()) {
        bad.push((
            Edit::Shop(ShopEdit {
                index: 0,
                items: shop_items(n),
            }),
            ObjectRef::Shop { index: 0 },
        ));
    }
    // A trainer whose team can't have 6, given one Pokémon more than it may have.
    if let Some(limits) = rules
        .trainers
        .trainers
        .iter()
        .find(|l| l.max_pokemon < 6 && l.max_pokemon > 0)
        && let Some(mut edit) = trainer_edit(data, limits.index)
        && let Some(last) = edit.pokemon.last().cloned()
    {
        while edit.pokemon.len() <= limits.max_pokemon {
            edit.pokemon.push(last.clone());
        }
        bad.push((Edit::Trainer(edit), ObjectRef::Trainer { index: limits.index }));
    }
    if let Some(t) = data.trades.first() {
        bad.push((
            Edit::Trade(TradeEdit {
                index: 0,
                requested: t.requested,
                given: t.given,
                nickname: "N".repeat(rules.trades.max_nickname_length + 1),
                ot_name: t.ot_name.clone(),
                ot_id: t.ot_id,
                ivs: t.ivs.clone(),
                held_item: t.held_item,
            }),
            ObjectRef::Trade { index: 0 },
        ));
    }
    if let Some(index) = rules.wild.levels.iter().position(|&l| l != WildLevels::Range)
        && data.encounters[index].encounters.len() > 1
    {
        let mut slots: Vec<_> = data.encounters[index]
            .encounters
            .iter()
            .map(|e| WildSlotEdit {
                species: e.species,
                min_level: e.min_level,
                max_level: e.max_level,
            })
            .collect();
        // Breaks one level per slot, a shared range and fixed levels alike.
        slots[1].min_level = slots[1].max_level.saturating_sub(1).max(1);
        slots[1].max_level = (slots[1].min_level + 2).min(100);
        bad.push((
            Edit::WildSet(WildSetEdit { index, slots }),
            ObjectRef::WildSet { index },
        ));
    }
    bad
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn edit_rules_fit_each_game() {
    let mut generations = std::collections::BTreeSet::new();
    for (path, mut handler) in roms() {
        let data = handler.game_data().clone();
        let rules = handler.edit_rules();
        let name = data.identity.name.clone();
        generations.insert(data.identity.generation.number());
        check_generation_rules(&data, &rules);
        assert_eq!(rules.wild.levels.len(), data.encounters.len(), "{name}");
        assert_eq!(rules.trainers.trainers.len(), data.trainers.len(), "{name}");

        // The game's own data fits its rules.
        let found = validate(&data, &rules, unchanged(&data));
        assert_eq!(found, [], "{name}: the game as loaded");

        // A few good edits pass, apply and are written.
        let mut good = Vec::new();
        if let Some(shop) = data.shops.first() {
            let mut items = shop.items.clone();
            items.reverse();
            good.push(Edit::Shop(ShopEdit { index: 0, items }));
        }
        if let Some(t) = data.trainers.iter().find(|t| !t.pokemon.is_empty())
            && let Some(mut edit) = trainer_edit(&data, t.index)
            && rules.trainer(t.index).is_some_and(|l| l.levels)
        {
            for p in &mut edit.pokemon {
                p.level = (p.level + 1).min(100);
            }
            good.push(Edit::Trainer(edit));
        }
        if let Some((index, &item)) = data
            .field_items
            .iter()
            .enumerate()
            .find(|(_, i)| !rules.field_items.required.contains(i))
            && let Some(&other) = data
                .field_items
                .iter()
                .find(|&&i| i != item && data.items.iter().any(|x| x.id == i))
        {
            good.push(Edit::FieldItem(FieldItemEdit { index, item: other }));
        }
        let found = validate(&data, &rules, good.clone());
        assert_eq!(found, [], "{name}: good edits");
        let list = EditList {
            edits: good,
            ..EditList::new(&data.identity)
        };
        let mut edited = data.clone();
        list.apply(&mut edited).expect("good edits apply");
        handler
            .apply(&edited)
            .unwrap_or_else(|e| panic!("{name}: good edits are written: {e}"));

        // Replacing a required field item is a warning.
        if let Some(index) = data
            .field_items
            .iter()
            .position(|i| rules.field_items.required.contains(i))
        {
            let other = data
                .field_items
                .iter()
                .copied()
                .find(|i| !rules.field_items.required.contains(i));
            if let Some(item) = other {
                let found = validate(&data, &rules, vec![Edit::FieldItem(FieldItemEdit { index, item })]);
                assert_eq!(found.len(), 1, "{name}");
                assert_eq!(found[0].severity, EditSeverity::Warning, "{name}");
            }
        }

        // Edits that break a rule are errors pointing at their object.
        for (edit, target) in bad_edits(&data, &rules) {
            let found = validate(&data, &rules, vec![edit]);
            assert!(
                found
                    .iter()
                    .any(|i| i.severity == EditSeverity::Error && i.target == Some(target)),
                "{name} ({}): {target} gives {found:?}",
                path.display()
            );
        }
        eprintln!("checked {name}");
    }
    assert!(!generations.is_empty(), "no supported ROMs in PPR_TEST_ROMS");
}
